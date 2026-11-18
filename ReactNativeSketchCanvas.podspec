require 'json'

package = JSON.parse(File.read(File.join(__dir__, 'package.json')))

Pod::Spec.new do |s|
  s.name           = 'ReactNativeSketchCanvas'
  s.module_name    = 'ReactNativeSketchCanvas'
  s.version        = package['version']
  s.summary        = package['description']
  s.description    = package['description']
  s.license        = { :type => 'Apache-2.0', :file => 'LICENSE' }
  s.author         = 'Apollo HG'
  s.homepage       = 'https://github.com/apollohg/react-native-sketch-canvas'
  s.platforms      = { :ios => '16.4' }
  s.swift_version  = '5.9'
  s.source         = { git: 'https://github.com/apollohg/react-native-sketch-canvas.git' }
  # UniFFI's Swift bindings import the `sketch_coreFFI` Clang module through a custom modulemap,
  # which CocoaPods only supports when the pod builds as a framework.
  s.static_framework = false

  s.dependency 'ExpoModulesCore'

  s.source_files = 'ios/*.swift'
  s.vendored_frameworks = 'ios/SketchCore.xcframework'
  s.preserve_paths = [
    'ios/sketch_coreFFI/**/*',
    'ios/SketchCore.xcframework/**/*',
  ]

  # Publish the FFI module in the pod's build directory, which dependents already search.
  s.script_phases = [{
    :name => 'Expose sketch_coreFFI',
    :execution_position => :before_compile,
    :script => <<~SCRIPT,
      set -e
      ffi_destination="${PODS_CONFIGURATION_BUILD_DIR}/ReactNativeSketchCanvas"
      mkdir -p "$ffi_destination"
      cp "${PODS_TARGET_SRCROOT}/ios/sketch_coreFFI/sketch_coreFFI.h" "$ffi_destination/"
      cp "${PODS_TARGET_SRCROOT}/ios/sketch_coreFFI/module.modulemap" "$ffi_destination/"
    SCRIPT
    :input_files => [
      '${PODS_TARGET_SRCROOT}/ios/sketch_coreFFI/sketch_coreFFI.h',
      '${PODS_TARGET_SRCROOT}/ios/sketch_coreFFI/module.modulemap',
    ],
    :output_files => [
      '${PODS_CONFIGURATION_BUILD_DIR}/ReactNativeSketchCanvas/sketch_coreFFI.h',
      '${PODS_CONFIGURATION_BUILD_DIR}/ReactNativeSketchCanvas/module.modulemap',
    ],
  }]

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_COMPILATION_MODE' => 'wholemodule',
    'SWIFT_INCLUDE_PATHS' => '$(PODS_CONFIGURATION_BUILD_DIR)/ReactNativeSketchCanvas',
    'HEADER_SEARCH_PATHS' => '$(PODS_CONFIGURATION_BUILD_DIR)/ReactNativeSketchCanvas',
  }
  s.user_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '$(PODS_CONFIGURATION_BUILD_DIR)/ReactNativeSketchCanvas',
  }
end
