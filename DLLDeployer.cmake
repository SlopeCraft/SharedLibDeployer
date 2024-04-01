cmake_minimum_required(VERSION 3.20)

# Replace backslash \ with slash /
function(DLLD_replace_backslash in_var out_var)
    set(temp)
    foreach (item ${${in_var}})
        string(REPLACE "\\" "/" item ${item})
        list(APPEND temp ${item})
    endforeach ()
    set(${out_var} ${temp} PARENT_SCOPE)
endfunction()

function(DLLD_get_deploy_dll_exe out_deploy_dll out_objdump)
    unset(${out_deploy_dll} PARENT_SCOPE)
    unset(${out_objdump} PARENT_SCOPE)

    set(extract_destination "${PROJECT_BINARY_DIR}/3rdParty/SharedLibDeployer")

    message(STATUS "Searching for installed deploy-dll executable")
    find_program(exe_path
        NAMES "deploy-dll"
        HINTS ${extract_destination}/bin)
    if(exe_path)
        message(STATUS "Found ${exe_path}")
        set(${out_deploy_dll} ${exe_path} PARENT_SCOPE)
    endif ()

    message(STATUS "Searching for objdump executable")
    find_program(objdump_path
        NAMES "objdump"
        HINTS ${extract_destination}/bin)
    if(objdump_path)
        message(STATUS "Found ${objdump_path}")
        set(${out_objdump} ${objdump_path} PARENT_SCOPE)
    endif ()

    if(EXISTS ${out_objdump} AND EXISTS ${out_deploy_dll})
        return()
    endif ()

    message(STATUS "Downloading and extracting SharedLibDeployer-compat-1.2.1-win64.7z")
    set(archive_loc "${PROJECT_BINARY_DIR}/SharedLibDeployer-compat-1.2.1-win64.7z")
    file(DOWNLOAD https://github.com/ToKiNoBug/SharedLibDeployer/releases/download/v1.2.1/SharedLibDeployer-compat-1.2.1-win64.7z
        ${archive_loc}
        EXPECTED_HASH SHA512=D2D8B8E269EC1C7178B05FE1A1E73E04F532A46314FC6BC1E8211189145B81609CCD763B78384F239878D5AE9F0AF7E0816472B1BFFB8BE8173BC5B934BBDE39)

    file(ARCHIVE_EXTRACT INPUT ${archive_loc} DESTINATION ${extract_destination})

    set(extracted_deploy_dll_exe "${extract_destination}/bin/deploy-dll.exe")
    if(NOT EXISTS ${extracted_deploy_dll_exe})
        message(FATAL_ERROR "${archive_loc} was extracted, but \"${extracted_deploy_dll_exe}\" was not found")
    endif ()
    DLLD_replace_backslash(extracted_deploy_dll_exe extracted_deploy_dll_exe)
    set(${out_deploy_dll} ${extracted_deploy_dll_exe})

    set(extracted_objdump_exe "${extract_destination}/bin/objdump.exe")
    if(NOT EXISTS ${extracted_objdump_exe})
        message(FATAL_ERROR "${archive_loc} was extracted, but \"${extracted_objdump_exe}\" was not found")
    endif ()
    DLLD_replace_backslash(extracted_objdump_exe extracted_objdump_exe)
    set(${out_objdump} ${extracted_objdump_exe})


#    message(STATUS "Searching for cargo")
#    find_program(cargo_path NAMES cargo)
#    if(cargo_path)
#        message(STATUS "Cloning ")
#    endif ()

endfunction()

DLLD_get_deploy_dll_exe(DLLD_deploy_dll_executable_location DLLD_objdump_executable_location)

function(DLLD_add_deploy target_name)
    cmake_parse_arguments(DLLD_add_deploy
            "BUILD_MODE;INSTALL_MODE;ALL"
            "INSTALL_DESTINATION"
            "IGNORE"
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

    set(ignore_tag "")
    foreach (ignore_dll_name ${DLLD_add_deploy_IGNORE})
        list(APPEND ignore_tag "--ignore=${ignore_dll_name}")
    endforeach ()

    DLLD_replace_backslash(CMAKE_PREFIX_PATH CMAKE_PREFIX_PATH)

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
            COMMAND ${DLLD_deploy_dll_executable_location} ${filename} ${ignore_tag} "--cmake-prefix-path=${CMAKE_PREFIX_PATH}" "--objdump-file=${DLLD_objdump_executable_location}"
            WORKING_DIRECTORY ${target_binary_dir})
        add_dependencies(${custom_target_name} ${target_name})
    endif ()

    if(${DLLD_add_deploy_INSTALL_MODE})

        install(CODE
            "
            message(STATUS \"CMAKE_SOURCE_DIR = \${CMAKE_SOURCE_DIR}\")
            message(STATUS \"CMAKE_INSTALL_PREFIX = \${CMAKE_INSTALL_PREFIX}\")
            execute_process(COMMAND ${DLLD_deploy_dll_executable_location} \"./${DLLD_add_deploy_INSTALL_DESTINATION}/${filename}\" ${ignore_tag} \"--cmake-prefix-path=${CMAKE_PREFIX_PATH}\" \"--objdump-file=${DLLD_objdump_executable_location}\"
                WORKING_DIRECTORY \${CMAKE_INSTALL_PREFIX}
                COMMAND_ERROR_IS_FATAL ANY)
            ")
    endif ()

endfunction()