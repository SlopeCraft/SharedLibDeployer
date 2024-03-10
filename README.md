# SharedLib Deployer

Deploy dlls for your exectuable, useful for redistributing your application as binaries.

## Usage
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