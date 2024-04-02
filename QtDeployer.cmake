

function(QD_add_deployqt target_name)
    if (NOT TARGET ${target_name})
        message(FATAL_ERROR "\"${target_name}\" is not a target")
    endif ()
    get_target_property(type ${target_name} TYPE)
    if (NOT ${type} STREQUAL "EXECUTABLE")
        message(FATAL_ERROR "\"${target_name}\" is not an executable")
    endif ()

    if (${WIN32})
        set(program_name "windeployqt")
        find_program(QD_deployqt_exe
                NAMES windeployqt
                REQUIRED)
    else ()
        set(program_name "macdeployqt")
        find_program(QD_deployqt_exe
                NAMES macdeployqt
                REQUIRED)
    endif ()

    cmake_parse_arguments(QD_add_deployqt
            "BUILD_MODE;INSTALL_MODE;ALL"
            "INSTALL_DESTINATION"
            "FLAGS"
            ${ARGN})

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
    if (${WIN32})
        set(filename "${filename}.exe")
    else ()
        set(filename "${filename}.app")
    endif ()

    set(tag_all )
    if(${QD_add_deployqt_ALL})
        set(tag_all ALL)
    endif ()

    set(flags "")
    if(QD_add_deployqt_FLAGS)
        list(APPEND flags ${QD_add_deployqt_FLAGS})
    endif ()

    if(${QD_add_deployqt_BUILD_MODE})
        if (${APPLE})
            message(WARNING "BUILD_MODE can not be used on apple because no macos bundle will be generated during compilation.")
        endif ()

        get_target_property(target_binary_dir ${target_name} BINARY_DIR)

        set(custom_target_name "QD_deploy_for_${target_name}")

        add_custom_target(${custom_target_name} ${tag_all}
            COMMAND ${QD_deployqt_exe} ${filename} ${flags}
            WORKING_DIRECTORY ${target_binary_dir}
            DEPENDS ${target_name}
            COMMENT "Run ${program_name} for ${target_name} at build directory"
            COMMAND_EXPAND_LISTS)

        if(NOT TARGET QD_deploy_all)
            add_custom_target(QD_deploy_all
                COMMENT "Build all targets like QD_deploy_for_*")
        endif ()
        add_dependencies(QD_deploy_all ${custom_target_name})

        if(TARGET "DLLD_deploy_for_${target_name}")
            add_dependencies("DLLD_deploy_for_${target_name}" ${custom_target_name})
        endif ()

    endif ()

    if(${QD_add_deployqt_INSTALL_MODE})
        string(JOIN " " flags ${flags})

        if (NOT DEFINED QD_add_deployqt_INSTALL_DESTINATION)
            message(FATAL_ERROR "INSTALL_DESTINATION must be assigned for INSTALL_MODE")
        endif ()

        cmake_path(IS_ABSOLUTE QD_add_deployqt_INSTALL_DESTINATION is_destination_abs)
        if (${is_destination_abs})
            message(FATAL_ERROR "Value passed to INSTALL_DESTINATION must be relative path, for example: \"bin\".")
        endif ()

        install(CODE
            "
            execute_process(COMMAND \"${QD_deployqt_exe}\" \"./${QD_add_deployqt_INSTALL_DESTINATION}/${filename}\" ${flags}
                WORKING_DIRECTORY \${CMAKE_INSTALL_PREFIX}
                COMMAND_ERROR_IS_FATAL ANY)
            ")
    endif ()

endfunction()