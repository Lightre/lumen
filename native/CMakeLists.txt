cmake_minimum_required(VERSION 3.24)
project(lumen_heic LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreaded)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE "${CMAKE_BINARY_DIR}/lib")
add_compile_options(/EHsc /MP4)
add_compile_definitions(LIBHEIF_STATIC_BUILD LIBDE265_STATIC_BUILD)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(ENABLE_DECODER OFF CACHE BOOL "" FORCE)
set(ENABLE_ENCODER OFF CACHE BOOL "" FORCE)
set(ENABLE_SDL OFF CACHE BOOL "" FORCE)
set(ENABLE_PLUGIN_LOADING OFF CACHE BOOL "" FORCE)
set(WITH_LIBDE265 ON CACHE BOOL "" FORCE)
foreach(option X265 X264 OpenH264_DECODER AOM_DECODER AOM_ENCODER LIBSHARPYUV
        EXAMPLES GDK_PIXBUF HEADER_COMPRESSION DAV1D SvtEnc RAV1E JPEG_DECODER
        JPEG_ENCODER OpenJPEG_DECODER OpenJPEG_ENCODER FFMPEG_DECODER VVDEC VVENC
        KVAZAAR UVG266 OPENJPH_ENCODER UNCOMPRESSED_CODEC)
    set(WITH_${option} OFF CACHE BOOL "" FORCE)
endforeach()
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(BUILD_DOCUMENTATION OFF CACHE BOOL "" FORCE)

# Check archive integrity and extract into the build directory, never the checkout.
file(READ "${CMAKE_CURRENT_SOURCE_DIR}/sources/manifest.json" manifest)
set(source_hashes "")
foreach(index RANGE 0 1)
    string(JSON archive GET "${manifest}" ${index} archive)
    string(JSON expected GET "${manifest}" ${index} sha256)
    file(SHA256 "${CMAKE_CURRENT_SOURCE_DIR}/sources/${archive}" actual)
    if(NOT actual STREQUAL expected)
        message(FATAL_ERROR "Native source hash mismatch: ${archive}")
    endif()
    string(APPEND source_hashes "${actual}")
endforeach()
set(previous_hashes "")
if(EXISTS "${CMAKE_BINARY_DIR}/sources/hashes.txt")
    file(READ "${CMAKE_BINARY_DIR}/sources/hashes.txt" previous_hashes)
endif()
if(NOT source_hashes STREQUAL previous_hashes)
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/sources")
    file(ARCHIVE_EXTRACT INPUT "${CMAKE_CURRENT_SOURCE_DIR}/sources/libheif-1.23.6.tar.gz"
         DESTINATION "${CMAKE_BINARY_DIR}/sources")
    file(ARCHIVE_EXTRACT INPUT "${CMAKE_CURRENT_SOURCE_DIR}/sources/libde265-1.1.3.tar.gz"
         DESTINATION "${CMAKE_BINARY_DIR}/sources")
    file(WRITE "${CMAKE_BINARY_DIR}/sources/hashes.txt" "${source_hashes}")
endif()
add_subdirectory("${CMAKE_BINARY_DIR}/sources/libde265" de265-build EXCLUDE_FROM_ALL)
set(LIBDE265_INCLUDE_DIR "${CMAKE_BINARY_DIR}/sources/libde265" CACHE PATH "" FORCE)
set(LIBDE265_LIBRARY de265 CACHE STRING "" FORCE)
add_subdirectory("${CMAKE_BINARY_DIR}/sources/libheif" heif-build EXCLUDE_FROM_ALL)
target_include_directories(heif PRIVATE "${CMAKE_BINARY_DIR}/de265-build")

add_library(lumen_heic STATIC heic.cpp)
target_include_directories(lumen_heic PRIVATE
    "${CMAKE_BINARY_DIR}/sources/libheif/libheif/api"
    "${CMAKE_BINARY_DIR}/heif-build")
target_link_libraries(lumen_heic PRIVATE heif de265)
