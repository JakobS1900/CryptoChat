use crate::overlay::{OverlayConfig, OverlayHandle, ReplicationEvent};
use cryptochat_messaging::EncryptedEnvelope;
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tokio::runtime::Runtime;

use jni::objects::{GlobalRef, JClass, JObject, JString, JValueGen};
use jni::sys::{jboolean, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use jni::JavaVM;

use serde_json::json;
use tracing::{error, info, warn};

struct AndroidNode {
    runtime: Runtime,
    handle: OverlayHandle,
    _vm: JavaVM,
}

static LOGGER: Lazy<()> = Lazy::new(|| {
    let _ = android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(tracing::Level::INFO)
            .with_tag("CryptoChat"),
    );
});

static NODE: Lazy<Mutex<Option<AndroidNode>>> = Lazy::new(|| Mutex::new(None));
static CALLBACK: Lazy<Mutex<Option<GlobalRef>>> = Lazy::new(|| Mutex::new(None));

fn with_node<F, R>(f: F) -> anyhow::Result<R>
where
    F: FnOnce(&AndroidNode) -> anyhow::Result<R>,
{
    let guard = NODE.lock().expect("android node mutex poisoned");
    let node = guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("CryptoChat node is not running"))?;
    f(node)
}

#[no_mangle]
pub extern "system" fn Java_com_cryptochat_node_Bridge_startNode(
    mut env: JNIEnv,
    _class: JClass,
    config_dir: JString,
) {
    Lazy::force(&LOGGER);

    if let Err(err) = start_node(&mut env, config_dir) {
        let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
    } else {
        info!("Android node start invoked");
    }
}

fn start_node(env: &mut JNIEnv, config_dir: JString) -> anyhow::Result<()> {
    let base: String = env.get_string(&config_dir)?.into();
    let storage_path = PathBuf::from(&base).join("cryptochat");

    let mut guard = NODE.lock().expect("android node mutex poisoned");
    if guard.is_some() {
        info!("CryptoChat node already running");
        return Ok(());
    }

    let vm = env.get_java_vm()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let config = OverlayConfig::default()
        .with_storage_path(storage_path)
        .with_retry_interval(Duration::from_secs(10));

    let handle = runtime.block_on(async { OverlayHandle::start(config).await })?;
    let mut rx = handle.subscribe_replication();
    let vm_clone = vm.clone();

    runtime.spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Err(err) = dispatch_event(&vm_clone, &event) {
                warn!(?err, "failed to dispatch replication event to Java");
            }
        }
    });

    *guard = Some(AndroidNode {
        runtime,
        handle,
        _vm: vm,
    });
    Ok(())
}

#[no_mangle]
pub extern "system" fn Java_com_cryptochat_node_Bridge_stopNode(env: JNIEnv, _class: JClass) {
    let mut guard = NODE.lock().expect("android node mutex poisoned");
    if let Some(node) = guard.take() {
        if let Err(err) = node.runtime.block_on(node.handle.shutdown()) {
            warn!(?err, "failed to shutdown node gracefully");
        }
        info!("CryptoChat node stopped");
    } else {
        warn!("stopNode called before startNode");
    }
    let _ = env;
}

#[no_mangle]
pub extern "system" fn Java_com_cryptochat_node_Bridge_registerCallback(
    mut env: JNIEnv,
    _class: JClass,
    callback: JObject,
) {
    match env.new_global_ref(callback) {
        Ok(global) => {
            *CALLBACK.lock().expect("callback mutex poisoned") = Some(global);
            info!("Registered replication callback");
        }
        Err(err) => error!(?err, "failed to register callback"),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cryptochat_node_Bridge_unregisterCallback(
    _env: JNIEnv,
    _class: JClass,
) {
    *CALLBACK.lock().expect("callback mutex poisoned") = None;
    info!("Unregistered replication callback");
}

#[no_mangle]
pub extern "system" fn Java_com_cryptochat_node_Bridge_publishEnvelope(
    mut env: JNIEnv,
    _class: JClass,
    envelope_json: JString,
) -> jboolean {
    match publish_envelope(&mut env, envelope_json) {
        Ok(_) => JNI_TRUE,
        Err(err) => {
            let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
            JNI_FALSE
        }
    }
}

fn dispatch_event(vm: &JavaVM, event: &ReplicationEvent) -> anyhow::Result<()> {
    let callback = {
        let guard = CALLBACK.lock().expect("callback mutex poisoned");
        guard.clone()
    };

    let Some(callback) = callback else {
        return Ok(());
    };

    let json = serde_json::to_string(&event_to_json(event))?;
    let mut env = vm.attach_current_thread()?;
    let jstr = env.new_string(json)?;
    env.call_method(
        callback.as_obj(),
        "onEvent",
        "(Ljava/lang/String;)V",
        &[JValueGen::Object(jstr.into())],
    )?;
    Ok(())
}

fn event_to_json(event: &ReplicationEvent) -> serde_json::Value {
    match event {
        ReplicationEvent::PublishQueued { message_id } => json!({
            "type": "queued",
            "messageId": message_id,
        }),
        ReplicationEvent::PublishAck { message_id, peer } => json!({
            "type": "ack",
            "messageId": message_id,
            "peer": peer.to_string(),
        }),
        ReplicationEvent::PublishFailed { message_id, reason } => json!({
            "type": "failed",
            "messageId": message_id,
            "reason": reason,
        }),
        ReplicationEvent::PublishRetry { message_id, peer } => json!({
            "type": "retry",
            "messageId": message_id,
            "peer": peer.to_string(),
        }),
    }
}

fn publish_envelope(env: &mut JNIEnv, envelope_json: JString) -> anyhow::Result<()> {
    let raw: String = env.get_string(&envelope_json)?.into();
    let envelope: EncryptedEnvelope = serde_json::from_str(&raw)
        .map_err(|err| anyhow::anyhow!("invalid envelope JSON: {err}"))?;

    with_node(|node| {
        let replication = node.handle.replication().clone();
        let publish_result = node
            .runtime
            .block_on(async move { replication.publish(envelope).await });
        publish_result?;
        Ok(())
    })
}
