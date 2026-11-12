# Both the ABI and JNI names must survive shrinking/obfuscation.
-keep class dev.verncat.player.soulseek.plugin.NativePlugin { *; }

# Resolve the host's callback interface, including invoke, through parent delegation.
-keep interface kotlin.jvm.functions.Function2 { *; }
