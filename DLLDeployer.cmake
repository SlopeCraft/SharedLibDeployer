cmake_minimum_required(VERSION 3.25)

set(DLLD_deploy_dll_exe_version "1.4.0")

# Replace backslash \ with slash /
function(DLLD_replace_backslash in_var out_var)
    set(temp)
    foreach (item ${${in_var}})
        string(REPLACE "\\" "/" item ${item})
        list(APPEND temp ${item})
    endforeach ()
    set(${out_var} ${temp} PARENT_SCOPE)
endfunction()

function(DLLD_validate_deploy_dll_exe exe_location out_error)
    unset(${out_error} PARENT_SCOPE)

    execute_process(COMMAND ${exe_location} --version
        OUTPUT_VARIABLE output
        COMMAND_ERROR_IS_FATAL ANY)

    string(REPLACE " " ";" splitted_output ${output})
    list(LENGTH splitted_output len)
    if(${len} LESS 2)
        set(${out_error} "Failed to deduce version from output \"${output}\"" PARENT_SCOPE)
        return()
    endif ()

    list(GET splitted_output 1 this_version)
    if(${this_version} VERSION_LESS ${DLLD_deploy_dll_exe_version})
        set(${out_error} "Required at least ${DLLD_deploy_dll_exe_version}, but found ${this_version}" PARENT_SCOPE)
        return()
    endif ()

    set(${out_error} "" PARENT_SCOPE)
endfunction()

function(DLLD_find_dll_deployer_validator validator_result_var item)
    DLLD_validate_deploy_dll_exe(${item} error)
    if(error)
        message(STATUS "Skip ${item} because ${error}")
        set(${validator_result_var} FALSE PARENT_SCOPE)
    else ()
        set(${validator_result_var} TRUE PARENT_SCOPE)
    endif ()
endfunction()

function(DLLD_get_deploy_dll_exe out_deploy_dll out_objdump)
    unset(${out_deploy_dll} PARENT_SCOPE)
    unset(${out_objdump} PARENT_SCOPE)

    set(extract_destination "${PROJECT_BINARY_DIR}/3rdParty/SharedLibDeployer")

    set(found_components 0)

    message(STATUS "Searching for installed deploy-dll executable")
    find_program(exe_path
        NAMES "deploy-dll"
        HINTS ${extract_destination}/bin
        VALIDATOR DLLD_find_dll_deployer_validator
        NO_CACHE)
    if(exe_path)
        message(STATUS "Found ${exe_path}")
        set(${out_deploy_dll} ${exe_path} PARENT_SCOPE)
        math(EXPR found_components "${found_components} + 1")
    endif ()

    message(STATUS "Searching for objdump executable")
    find_program(objdump_path
        NAMES "objdump"
        HINTS ${extract_destination}/bin
        NO_CACHE)
    if(objdump_path)
        message(STATUS "Found ${objdump_path}")
        set(${out_objdump} ${objdump_path} PARENT_SCOPE)
        math(EXPR found_components "${found_components} + 1")
    endif ()

    if(${found_components} GREATER_EQUAL 2)
        return()
    endif ()

    message(STATUS "Downloading and extracting SharedLibDeployer-${DLLD_deploy_dll_exe_version}-win64.7z")
    set(archive_loc "${PROJECT_BINARY_DIR}/SharedLibDeployer-${DLLD_deploy_dll_exe_version}-win64.7z")
    file(DOWNLOAD https://github.com/ToKiNoBug/SharedLibDeployer/releases/download/v${DLLD_deploy_dll_exe_version}/SharedLibDeployer-${DLLD_deploy_dll_exe_version}-win64.7z
        ${archive_loc}
        EXPECTED_HASH SHA512=5B2AC756A922BE06E0EFE88AA157A8A626B62A64D5EC3B19BF65A3581F3E98515B29BC21F9AB346DA4C6F35A4104364780144100645534DCD123867C02297755)

    file(ARCHIVE_EXTRACT INPUT ${archive_loc} DESTINATION ${extract_destination})

    set(extracted_deploy_dll_exe "${extract_destination}/bin/deploy-dll.exe")
    if(NOT EXISTS ${extracted_deploy_dll_exe})
        message(FATAL_ERROR "${archive_loc} was extracted, but \"${extracted_deploy_dll_exe}\" was not found")
    endif ()
    DLLD_replace_backslash(extracted_deploy_dll_exe extracted_deploy_dll_exe)
    set(${out_deploy_dll} ${extracted_deploy_dll_exe} PARENT_SCOPE)

    set(extracted_objdump_exe "${extract_destination}/bin/objdump.exe")
    if(NOT EXISTS ${extracted_objdump_exe})
        message(FATAL_ERROR "${archive_loc} was extracted, but \"${extracted_objdump_exe}\" was not found")
    endif ()
    DLLD_replace_backslash(extracted_objdump_exe extracted_objdump_exe)
    set(${out_objdump} ${extracted_objdump_exe} PARENT_SCOPE)


#    message(STATUS "Searching for cargo")
#    find_program(cargo_path NAMES cargo)
#    if(cargo_path)
#        message(STATUS "Cloning ")
#    endif ()

endfunction()

DLLD_get_deploy_dll_exe(DLLD_deploy_dll_executable_location DLLD_objdump_executable_location)

function(DLLD_add_deploy target_name)
    cmake_parse_arguments(DLLD_add_deploy
            "BUILD_MODE;INSTALL_MODE;ALL;VERBOSE;COPY_VC_REDIST"
            "INSTALL_DESTINATION"
            "IGNORE;OPTIONAL_DLLS;FLAGS"
            ${ARGN})

    # Check target type
    get_target_property(target_type ${target_name} TYPE)
    set(valid_types EXECUTABLE SHARED_LIBRARY)
    if(NOT ${target_type} IN_LIST valid_types)
        message(FATAL_ERROR "The type of ${target_name} is invalid. Valid types: ${valid_types}")
    endif ()

    # Get filename of target
    get_target_property(target_prefix ${target_name} PREFIX)
    get_target_property(target_prop_name ${target_name} NAME)
    get_target_property(target_suffix ${target_name} SUFFIX)
    set(filename ${target_prop_name})
    if(target_prefix)
        set(filename "${target_prefix}${filename}")
    endif ()
    if(target_suffix)
        set(filename "${filename}${target_suffix}")
    endif ()
    if(${target_type} STREQUAL EXECUTABLE)
        set(filename "${filename}.exe")
    else ()
        set(filename "${filename}.dll")
    endif ()

    set(flags "")
    foreach (ignore_dll_name ${DLLD_add_deploy_IGNORE})
        list(APPEND flags "--ignore=${ignore_dll_name}")
    endforeach ()

    if(${DLLD_add_deploy_VERBOSE})
        list(APPEND flags "--verbose")
    endif ()

    if(${DLLD_add_deploy_COPY_VC_REDIST})
        list(APPEND flags "--copy-vc-redist")
    endif ()

    cmake_path(GET CMAKE_C_COMPILER PARENT_PATH c_compiler_path)
    if(c_compiler_path)
        list(APPEND flags "\"--shallow-search-dir=${c_compiler_path}\"")
    endif ()
    cmake_path(GET CMAKE_CXX_COMPILER PARENT_PATH cxx_compiler_path)
    if((cxx_compiler_path) AND (NOT c_compiler_path STREQUAL cxx_compiler_path))
        list(APPEND flags "\"--shallow-search-dir=${cxx_compiler_path}\"")
    endif ()

    list(APPEND flags "\"--deep-search-dir=${CMAKE_BINARY_DIR}\"")

    foreach (item in ${DLLD_add_deploy_OPTIONAL_DLLS})
        list(APPEND flags "\"--optional-dlls=${item}\"")
    endforeach ()

    DLLD_replace_backslash(CMAKE_PREFIX_PATH CMAKE_PREFIX_PATH)

    foreach (path ${CMAKE_PREFIX_PATH})
        list(APPEND flags "\"--cmake-prefix-path=${path}\"")
    endforeach ()

    list(APPEND flags ${DLLD_add_deploy_FLAGS})

    if(${DLLD_add_deploy_BUILD_MODE})
        set(custom_target_name "DLLD_deploy_for_${target_name}")
        if (${DLLD_add_deploy_ALL})
            set(DLLD_all_tag ALL)
        else ()
            set(DLLD_all_tag)
        endif ()

        get_target_property(target_binary_dir ${target_name} BINARY_DIR)

        add_custom_target(${custom_target_name}
            ${DLLD_all_tag}
            COMMAND ${DLLD_deploy_dll_executable_location} ${filename} --objdump-file=${DLLD_objdump_executable_location} ${flags}
            WORKING_DIRECTORY ${target_binary_dir}
            DEPENDS ${target_name}
            COMMENT "Deploy dll for ${target_name} at build directory"
            COMMAND_EXPAND_LISTS)

        if(NOT TARGET DLLD_deploy_all)
            add_custom_target(DLLD_deploy_all
                COMMENT "Build all targets like DLLD_deploy_for_*")
        endif ()
        add_dependencies(DLLD_deploy_all ${custom_target_name})

        if(TARGET "QD_deploy_for_${target_name}")
            add_dependencies(${custom_target_name} "QD_deploy_for_${target_name}")
        endif ()
    endif ()

    if(${DLLD_add_deploy_INSTALL_MODE})

        string(JOIN " " flags ${flags})

        install(CODE
            "
            execute_process(COMMAND \"${DLLD_deploy_dll_executable_location}\" \"./${DLLD_add_deploy_INSTALL_DESTINATION}/${filename}\" \"--objdump-file=${DLLD_objdump_executable_location}\" ${flags}
                WORKING_DIRECTORY \${CMAKE_INSTALL_PREFIX}
                COMMAND_ERROR_IS_FATAL ANY)
            ")
    endif ()

endfunction()