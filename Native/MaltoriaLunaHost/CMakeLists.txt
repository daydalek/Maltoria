cmake_minimum_required(VERSION 3.25)
project(MaltoriaLunaHost LANGUAGES CXX)

if(NOT WIN32)
    message(FATAL_ERROR "MaltoriaLunaHost is a Windows-only helper")
endif()
if(NOT EXISTS "${NLOHMANN_JSON_INCLUDE_DIR}/nlohmann/json.hpp")
    message(FATAL_ERROR "Set NLOHMANN_JSON_INCLUDE_DIR to the pinned single_include directory")
endif()

if(CMAKE_SIZEOF_VOID_P EQUAL 4)
    set(MALTORIA_ARCH x86)
    set(MALTORIA_BITS 32)
else()
    set(MALTORIA_ARCH x64)
    set(MALTORIA_BITS 64)
endif()

add_executable(MaltoriaLunaHost WIN32 src/main.cpp)
target_compile_features(MaltoriaLunaHost PRIVATE cxx_std_20)
target_compile_definitions(MaltoriaLunaHost PRIVATE
    WIN32_LEAN_AND_MEAN NOMINMAX _WIN32_WINNT=0x0A00 MALTORIA_ARCH="${MALTORIA_ARCH}")
target_include_directories(MaltoriaLunaHost SYSTEM PRIVATE "${NLOHMANN_JSON_INCLUDE_DIR}")
target_link_libraries(MaltoriaLunaHost PRIVATE ws2_32)
set_target_properties(MaltoriaLunaHost PROPERTIES
    OUTPUT_NAME "MaltoriaLunaHost${MALTORIA_BITS}"
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

add_executable(MaltoriaUnicodeFixture WIN32 tests/UnicodeFixture.cpp)
target_compile_features(MaltoriaUnicodeFixture PRIVATE cxx_std_17)
target_compile_definitions(MaltoriaUnicodeFixture PRIVATE
    WIN32_LEAN_AND_MEAN NOMINMAX UNICODE _UNICODE)
target_link_libraries(MaltoriaUnicodeFixture PRIVATE user32 gdi32)
set_target_properties(MaltoriaUnicodeFixture PROPERTIES
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

if(MSVC)
    foreach(target MaltoriaLunaHost MaltoriaUnicodeFixture)
        target_compile_options(${target} PRIVATE /W4 /utf-8 /permissive-)
    endforeach()
endif()
