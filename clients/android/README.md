# CryptoChat Android prototype

This folder hosts a Compose-based Android shell plus a placeholder JNI bridge for the Rust node.

## Building
```bash
cd clients/android
./gradlew app:assembleDebug
```

The resulting APK lives at `app/build/outputs/apk/debug/app-debug.apk` and should install on a Pixel 7 Pro running GrapheneOS (minSdk 26, targetSdk 34).

## Next steps
- Implement JNI bindings in `node/` to call into the Rust overlay runtime.
- Wire the broadcast replication events into the Compose UI.
- Add instrumentation tests under `app/src/androidTest`.

### Native library

To exercise the JNI hooks, build the Rust node as an Android shared library (for example arm64):

```bash
cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --release
```

This produces `app/src/main/jniLibs/arm64-v8a/libcryptochat_node.so`, which `Bridge.startNode` loads at runtime.
