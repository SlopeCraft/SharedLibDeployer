cmake_minimum_required(VERSION 3.20)
project(DLLDeployerWindowsTest LANGUAGES C CXX VERSION 1.2.2)

add_executable(test test.cpp)

find_package(libzip CONFIG REQUIRED)
target_link_libraries(test PRIVATE libzip::zip)

find_package(OpenMP REQUIRED)
target_link_libraries(test PRIVATE OpenMP::OpenMP_CXX)

find_package(Qt6 COMPONENTS Core Widgets REQUIRED)
target_link_libraries(test PRIVATE Qt6::Core Qt6::Widgets)
target_compile_definitions(test PRIVATE QT_SUPPORT=1)

include(../DLLDeployer.cmake)
include(../QtDeployer.cmake)

QD_add_deployqt(test BUILD_MODE FLAGS "-release;--no-translations")
DLLD_add_deploy(test ALL BUILD_MODE VERBOSE COPY_VC_REDIST)

install(TARGETS test
    RUNTIME DESTINATION bin)
QD_add_deployqt(test INSTALL_MODE
        INSTALL_DESTINATION bin
        FLAGS "-release;--no-translations")
DLLD_add_deploy(test INSTALL_MODE COPY_VC_REDIST
        INSTALL_DESTINATION bin)

include(CPack)