# SharedLibDeployer

Deploy dlls for your exectuable, useful for redistributing your application as binaries.

## Usage

### CMake wrapper

`DLLDeployer.cmake` provides function `DLLD_add_deploy`, enabling developers to run `deploy-dll.exe` in both build and install directory automatically.

This repository also provides `QtDeployer.cmake`, which implements `QD_add_deployqt` to run `windeployqt/macdeployqt` in both build and install directory.

#### Example:

```cmake
file(DOWNLOAD https://github.com/SlopeCraft/SharedLibDeployer/releases/latest/download/DLLDeployer.cmake
    ${CMAKE_BINARY_DIR}/DLLDeployer.cmake)
file(DOWNLOAD https://github.com/SlopeCraft/SharedLibDeployer/releases/latest/download/QtDeployer.cmake
        ${CMAKE_BINARY_DIR}/QtDeployer.cmake)
include(${CMAKE_BINARY_DIR}/DLLDeployer.cmake)
include(${CMAKE_BINARY_DIR}/QtDeployer.cmake)

add_executable(your_exe <your/source/files>)
# Run windeployqt/macdeployqt at current binary dir
QD_add_deployqt(test BUILD_MODE FLAGS "-release;--no-translations")
# Deploy dlls at current binary dir
DLLD_add_deploy(your_exe BUILD_MODE
    VERBOSE # Show detailed information
)

install(TARGETS your_exe RUNTIME DESTINATION bin)
QD_add_deployqt(test INSTALL_MODE
        INSTALL_DESTINATION bin
        FLAGS "-release;--no-translations")
# Deploy dlls at installation dir, useful when distributing binaries
DLLD_add_deploy(your_exe INSTALL_MODE
    INSTALL_DESTINATION bin # Where you install the exe
    COPY_VC_REDIST # Copy Microsoft Visual C++ redistributable binaries, By default it is turned off
)
```

A detailed example is [here](./tests/CMakeLists.txt)

#### Function prototype:
```cmake
DLLD_add_deploy(target_name 
    [BUILD_MODE] [INSTALL_MODE] [ALL] [VERBOSE] [COPY_VC_REDIST]
    [INSTALL_DESTINATION path/of/install/prefix]
    [IGNORE ignored dll names accept;list]
    [OPTIONAL_DLLS relative/path/to/optional/dlls;accept/list]
    [FLAGS --any-extra-arguments-passed-to-deploy-dll.exe;--accept-lists]
)

QD_add_deployqt(target_name
    [BUILD_MODE] [INSTALL_MODE] [ALL]
    [INSTALL_DESTINATION path/of/install/prefix]
    [FLAGS --any-extra-arguments-passed-to-windeployqt.exe-or-macdeployqt;--accept-lists]
)
```

#### Custom targets

For each target(let's call is `A`), DLLDeployer will create a custom target named `DLLD_deploy_for_A`(for QtDeployer, it is `QD_deploy_for_A`). Both `DLLD_deploy_for_A` and `QD_deploy_for_A` depend on `A`, ensuring the executable/shared lib exists when deploying. If both custom targets exist, `DLLD_deploy_for_A` will depend on `QD_deploy_for_A`. 

There will be 2 helper custom targets `DLLD_deploy_all` and `QD_deploy_all`. Every target named `DLLD_deploy_for_*` depend on `DLLD_deploy_all`, and every target named `QD_deploy_for_*` depend on `QD_deploy_all`. There custom targets is not included in `ALL`, so they won't be built unless you tell cmake to build them explicitly.

#### Best Practice

You won't need QtDeployer if you are not using Qt. DLLDeployer is what you need.

For Qt-based executables, it's strongly suggested to use QtDeployer together with DLLDeployer. Shared lib is not the only thing we need to deploy, so deploy-dll.exe is not able to take the place of `windeployqt`/`macdeployqt`. However, the latter will only deploy qt dlls. So we should run `windeployqt` firstly, and `deploy-dll` secondly.

Don't deploy VC redistributable dlls unless you are developing an application for muggles. Users should learn to install VC runtime on their Windows.

Here's some notice and suggestions:

1. Install mode
   1. In cmake script, we should call 3 functions: `install`, `QD_add_deployqt` and `DLLD_add_deploy`. You must call them on your target in order so that `windeployqt` and `deploy-dll` can work normally. Drop `QD_add_deployqt` if you are not using Qt.
   2. The `INSTALL_DESTINATION` passed to `QD_add_deployqt` and `DLLD_add_deploy` should be the same as `RUNTIME DESTINATION` passed to `install`. Install destination should be a RELATIVE path like `bin` or `.`, and you don't have to add prefix like `./`. **Do NOT use absolute path**, this is incompatible with CPack.

2. Build mode
   1. Deploy dll in build dir is only an assistance for developing, it has no effect on the installation procedure.
   2. Use custom targets instead of `ALL`. 
   3. VS generators are not perfectly supported, it is caused by different behaviors:
      1. With VS generators, binaries will be put at `${CMAKE_CURRENT_BINARY_DIR}/${CMAKE_BUILD_TYPE}`, but for many other generators the binary is put directly at `${CMAKE_CURRENT_BINARY_DIR}`. The latter is expected.
      2. When we use cmake with VS generators, there's no way to exclude a target from ALL. When you build the whole project, all custom targets will be built regardless of your willing.
      3. Install mode works perfectly, only build mode will be affected.

### Command Line Executable
```shell
deploy-dll.exe C:/path/to/your/executable.exe
deploy-dll.exe C:/path/to/your/shared/lib.dll
```

```text
Usage: deploy-dll.exe [OPTIONS] <BINARY_FILE>

Arguments:
  <BINARY_FILE>
          The target file to deploy dll for. This can be an exe or dll

Options:
      --skip-env-path
          No not search in system variable PATH

      --copy-vc-redist
          Copy Microsoft Visual C/C++ redistributable dlls

      --verbose
          Show verbose information during execution

      --shallow-search-dir <SHALLOW_SEARCH_DIR>
          Search for dll in those dirs

      --no-shallow-search
          Disable shallow search

      --deep-search-dir <DEEP_SEARCH_DIR>
          Search for dll recursively in those dirs

      --no-deep-search
          Disable recursive search

      --cmake-prefix-path <CMAKE_PREFIX_PATH>
          CMAKE_PREFIX_PATH for cmake to search for packages

      --ignore <IGNORE>
          Dll files that won't be deployed

      --objdump-file <OBJDUMP_FILE>
          Location of dumpbin file

          [default: [builtin]]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
