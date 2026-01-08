#![cfg(feature = "dns-client")]

use core::{
    cell::UnsafeCell, future::Future, net::Ipv6Addr, ptr::null, str::FromStr,
    sync::atomic::Ordering, task::Poll,
};

use alloc::{ffi::CString, sync::Arc};
use embassy_sync::waitqueue::AtomicWaker;
use openthread_sys::{
    otDnsAddressResponse, otDnsAddressResponseGetAddress, otDnsClientResolveAddress,
    otDnsQueryConfig, otDnsRecursionFlag_OT_DNS_FLAG_NO_RECURSION, otDnsServiceInfo,
    otDnsServiceMode_OT_DNS_SERVICE_MODE_SRV_TXT_SEPARATE,
    otDnsServiceMode_OT_DNS_SERVICE_MODE_TXT, otDnsServiceResponse,
    otDnsServiceResponseGetHostAddress, otDnsServiceResponseGetServiceInfo,
    otDnsServiceResponseGetServiceName, otDnsTransportProto_OT_DNS_TRANSPORT_TCP, otError,
    otError_OT_ERROR_NONE, otIp6Address,
};
use portable_atomic::AtomicBool;

use crate::{sys::otDnsClientResolveServiceAndHostAddress, OpenThread};

pub struct DnsClient<'a> {
    ot: OpenThread<'a>,
}

impl<'a> DnsClient<'a> {
    pub fn new(ot: OpenThread<'a>) -> DnsClient<'a> {
        DnsClient { ot }
    }

    pub fn resolve_service_and_host_address(
        &self,
        instance_label: &str,
        service_name: &str,
    ) -> DnsFuture<DnsResolutionResponse> {
        let dns_future_state = Arc::new(DnsFutureState {
            ready: AtomicBool::new(false),
            result: UnsafeCell::new(None),
            waker: AtomicWaker::new(),
        });

        let context_pointer = Arc::into_raw(dns_future_state.clone()) as *mut core::ffi::c_void;

        let mut openthread = self.ot.activate();
        let state = openthread.state();

        unsafe {
            let mut config: otDnsQueryConfig = core::mem::zeroed();
            config.mServiceMode = otDnsServiceMode_OT_DNS_SERVICE_MODE_SRV_TXT_SEPARATE;
            // config.mRecursionFlag = otDnsRecursionFlag_OT_DNS_FLAG_NO_RECURSION;

            let instance_label = CString::from_str(instance_label).unwrap();
            let service_name = CString::from_str(service_name).unwrap();
            otDnsClientResolveServiceAndHostAddress(
                state.ot.instance,
                instance_label.as_ptr(),
                service_name.as_ptr(),
                Some(resolve_service_and_host_address_callback),
                context_pointer,
                &config,
                // null(),
            );

            // TODO: Handle an error response that comes directly from above function call, and don't allow caller to wait indefinitely (test by passing null instance label)
        }

        DnsFuture {
            state: dns_future_state,
        }
    }

    pub fn resolve_host_address(&self, host_name: &str) -> DnsFuture<DnsResolutionResponse> {
        let dns_future_state = Arc::new(DnsFutureState {
            ready: AtomicBool::new(false),
            result: UnsafeCell::new(None),
            waker: AtomicWaker::new(),
        });

        let context_pointer = Arc::into_raw(dns_future_state.clone()) as *mut core::ffi::c_void;

        let mut openthread = self.ot.activate();
        let state = openthread.state();

        unsafe {
            // let mut config: otDnsQueryConfig = core::mem::zeroed();
            // config.mServiceMode = otDnsServiceMode_OT_DNS_SERVICE_MODE_TXT;
            // config.mRecursionFlag = otDnsRecursionFlag_OT_DNS_FLAG_NO_RECURSION;

            let host_name_cstring = CString::from_str(host_name).unwrap();

            otDnsClientResolveAddress(
                state.ot.instance,
                host_name_cstring.as_ptr(),
                Some(resolve_host_address_callback),
                context_pointer,
                // &config,
                null(),
            );
            // otDnsClientResolveServiceAndHostAddress(
            //     state.ot.instance,
            //     instance_label.as_ptr(),
            //     service_name.as_ptr(),
            //     Some(resolve_service_and_host_address_callback),
            //     context_pointer,
            //     &config,
            // );
        }

        DnsFuture {
            state: dns_future_state,
        }
    }
}

unsafe extern "C" fn resolve_service_and_host_address_callback(
    error: otError,
    response: *const otDnsServiceResponse,
    context_pointer: *mut ::core::ffi::c_void,
) {
    let state = Arc::from_raw(context_pointer as *const DnsFutureState<DnsResolutionResponse>);

    // otDnsServiceResponseGetHostAddress(aResponse, aHostName, aIndex, aAddress, aTtl)
    let mut service_info: otDnsServiceInfo = core::mem::zeroed();

    if error == otError_OT_ERROR_NONE {
        otDnsServiceResponseGetServiceInfo(response, &mut service_info);
        // otDnsServiceResponseGetServiceName(aResponse, aLabelBuffer, aLabelBufferSize, aNameBuffer, aNameBufferSize)

        let ip_address = Ipv6Addr::from_octets(service_info.mHostAddress.mFields.m8);
        let port = service_info.mPort;
        info!(
            "resolve_service_and_host_address_callback, ip_address={}, port={}",
            ip_address, port
        );
        *state.result.get() = Some(DnsResolutionResponse { ip_address, port });
    } else {
        error!("resolve_service_and_host_address_callback, error={}", error);
        *state.result.get() = Some(DnsResolutionResponse {
            ip_address: Ipv6Addr::from_bits(0),
            port: 0,
        });
    }
    state.ready.store(true, Ordering::Release);
    state.waker.wake();
}

unsafe extern "C" fn resolve_host_address_callback(
    error: otError,
    response: *const otDnsAddressResponse,
    context_pointer: *mut ::core::ffi::c_void,
) {
    info!("resolve_service_and_host_address_callback, error={}", error);
    let state = Arc::from_raw(context_pointer as *const DnsFutureState<DnsResolutionResponse>);

    let mut ot_ip6_address: otIp6Address = core::mem::zeroed();
    let mut ttl: u32 = 0;
    otDnsAddressResponseGetAddress(response, 0, &mut ot_ip6_address, &mut ttl);

    let ip_address = Ipv6Addr::from_octets(ot_ip6_address.mFields.m8);
    let port = 1212;
    info!("resolve_host_address_callback, ip_address={}", ip_address);
    *state.result.get() = Some(DnsResolutionResponse { ip_address, port });
    state.ready.store(true, Ordering::Release);
    state.waker.wake();
}

#[derive(Clone, Copy)]
pub struct DnsResolutionResponse {
    // TODO: This needs to take into account the error state
    pub ip_address: Ipv6Addr,
    pub port: u16,
}

pub struct DnsFuture<T: Copy> {
    state: alloc::sync::Arc<DnsFutureState<T>>,
}

impl<T: Copy> Future for DnsFuture<T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.state.waker.register(cx.waker());

        if self.state.ready.load(Ordering::Acquire) {
            let result = unsafe { (*self.state.result.get()).unwrap() };
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    }
}

struct DnsFutureState<T> {
    ready: AtomicBool,
    result: UnsafeCell<Option<T>>,
    waker: AtomicWaker,
}

unsafe impl<T> Sync for DnsFutureState<T> {}
unsafe impl<T> Send for DnsFutureState<T> {}
