cmake_minimum_required(VERSION 3.20)
project(DLLDeployerWindowsTest LANGUAGES C CXX VERSION 1.2.2)

add_executable(test test.cpp)

find_package(libzip CONFIG REQUIRED)
target_link_libraries(test PRIVATE libzip::zip)

find_package(OpenMP REQUIRED)
target_link_libraries(test PRIVATE OpenMP::OpenMP_CXX)

include(../DLLDeployer.cmake)

DLLD_add_deploy(test ALL BUILD_MODE VERBOSE COPY_VC_REDIST)

install(TARGETS test
    RUNTIME DESTINATION bin)
DLLD_add_deploy(test INSTALL_MODE COPY_VC_REDIST
        INSTALL_DESTINATION bin)

include(CPack)