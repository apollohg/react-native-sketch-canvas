# JNA references desktop AWT helpers that Android lacks; the bindings never call them.
-dontwarn java.awt.**

# UniFFI binds Kotlin names to the Rust library through JNA reflection.
-keep class com.sun.jna.** { *; }
-keep class uniffi.sketch_core.** { *; }
-keepattributes Signature,InnerClasses,EnclosingMethod,*Annotation*
-keepclassmembers class * implements com.sun.jna.Library { *; }
-keepclassmembers class * extends com.sun.jna.Structure { *; }
