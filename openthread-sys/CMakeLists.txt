cmake_minimum_required(VERSION 3.16.0)
project(openthread VERSION 1.0.0)

# To avoid name clashes if these C functions are defined by some other library
add_definitions(-Dsnprintf=_support_snprintf -Dvsnprintf=_support_vsnprintf)

# OpenThread itself
add_subdirectory(./openthread)

# Custom C code (only `snprintf`/`vsnprintf` currently)
add_subdirectory(./gen/support)


# set(OT_SYS_FLASH_API_ENABLE "" CACHE STRING "define OPENTHREAD_CONFIG_PLATFORM_FLASH_API_ENABLE for OpenThread")
# set_property(CACHE OT_SYS_FLASH_API_ENABLE PROPERTY STRINGS)
# if (${OT_SYS_FLASH_API_ENABLE})
#     message(STATUS "OT_SYS_FLASH_API_ENABLE=${OT_SYS_FLASH_API_ENABLE} -> OPENTHREAD_CONFIG_PLATFORM_FLASH_API_ENABLE=${OT_SYS_FLASH_API_ENABLE}")
#     target_compile_definitions(ot-config INTERFACE
#         "OPENTHREAD_CONFIG_PLATFORM_FLASH_API_ENABLE=${OT_SYS_FLASH_API_ENABLE}"
#     )
# endif
