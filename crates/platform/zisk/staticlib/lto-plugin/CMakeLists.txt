cmake_minimum_required(VERSION 3.20)
project(zisk_dma C CXX)

# The plugin runs inside `ld.lld`, so it must be built against the same LLVM (LLVM_DIR).
find_package(LLVM REQUIRED CONFIG)
if(NOT LLVM_VERSION_MAJOR EQUAL 22)
  message(FATAL_ERROR "zisk-dma needs LLVM 22, found ${LLVM_PACKAGE_VERSION} at ${LLVM_DIR}")
endif()

add_library(zkvm-lto-plugin MODULE ZiskDma.cpp)
set_target_properties(zkvm-lto-plugin PROPERTIES PREFIX "" SUFFIX ".so" CXX_STANDARD 17)
target_include_directories(zkvm-lto-plugin SYSTEM PRIVATE ${LLVM_INCLUDE_DIRS})
separate_arguments(LLVM_DEFINITIONS_LIST NATIVE_COMMAND ${LLVM_DEFINITIONS})
target_compile_definitions(zkvm-lto-plugin PRIVATE ${LLVM_DEFINITIONS_LIST})
if(NOT LLVM_ENABLE_RTTI)
  target_compile_options(zkvm-lto-plugin PRIVATE -fno-rtti)
endif()
# LLVM's symbols come from the linker process that loads the plugin.
if(APPLE)
  target_link_options(zkvm-lto-plugin PRIVATE -undefined dynamic_lookup)
endif()
