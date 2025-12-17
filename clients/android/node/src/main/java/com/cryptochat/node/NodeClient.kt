package com.cryptochat.node

import android.util.Log
import androidx.annotation.Keep
import java.util.UUID
import java.util.concurrent.CopyOnWriteArrayList
import org.json.JSONException
import org.json.JSONObject

@Keep
sealed class ReplicationEventPayload(open val messageId: String) {
    @Keep
    data class Queued(override val messageId: String) : ReplicationEventPayload(messageId)

    @Keep
    data class Ack(override val messageId: String, val peer: String) : ReplicationEventPayload(messageId)

    @Keep
    data class Failed(override val messageId: String, val reason: String) : ReplicationEventPayload(messageId)

    @Keep
    data class Retry(override val messageId: String, val peer: String) : ReplicationEventPayload(messageId)
}

object NodeClient {
    private const val TAG = "CryptoChat"

    private val listeners = CopyOnWriteArrayList<(ReplicationEventPayload) -> Unit>()

    private val nativeCallback = object : EventCallback {
        override fun onEvent(payload: String) {
            val event = parseReplicationEvent(payload) ?: return
            listeners.forEach { listener ->
                runCatching { listener(event) }.onFailure { error ->
                    Log.w(TAG, "Listener threw while handling replication event", error)
                }
            }
        }
    }

    fun start(configDir: String) {
        try {
            Bridge.startNode(configDir)
        } catch (t: Throwable) {
            Log.e(TAG, "Failed to start native node", t)
            throw t
        }
    }

    fun stop() {
        try {
            Bridge.unregisterCallback()
        } catch (t: Throwable) {
            Log.w(TAG, "Failed to unregister callback", t)
        }
        listeners.clear()
        try {
            Bridge.stopNode()
        } catch (t: Throwable) {
            Log.e(TAG, "Failed to stop native node", t)
        }
    }

    fun addReplicationListener(listener: (ReplicationEventPayload) -> Unit) {
        listeners.add(listener)
        if (listeners.size == 1) {
            try {
                Bridge.registerCallback(nativeCallback)
            } catch (t: Throwable) {
                Log.e(TAG, "Failed to register replication callback", t)
                listeners.remove(listener)
                throw t
            }
        }
    }

    fun removeReplicationListener(listener: (ReplicationEventPayload) -> Unit) {
        listeners.remove(listener)
        if (listeners.isEmpty()) {
            try {
                Bridge.unregisterCallback()
            } catch (t: Throwable) {
                Log.w(TAG, "Failed to unregister replication callback", t)
            }
        }
    }

    fun publishEnvelope(envelopeJson: String): Boolean {
        return try {
            Bridge.publishEnvelope(envelopeJson)
        } catch (t: Throwable) {
            Log.e(TAG, "Failed to publish envelope via JNI", t)
            false
        }
    }

    private fun parseReplicationEvent(payload: String): ReplicationEventPayload? {
        return try {
            val root = JSONObject(payload)
            val type = root.optString("type")
            val messageId = root.optString("messageId")
            if (type.isNullOrEmpty() || messageId.isNullOrEmpty()) {
                Log.w(TAG, "Replication event missing type or messageId: $payload")
                return null
            }
            when (type) {
                "queued" -> ReplicationEventPayload.Queued(messageId)
                "ack" -> {
                    val peer = root.optString("peer")
                    if (peer.isNullOrEmpty()) {
                        Log.w(TAG, "Ack event missing peer: $payload")
                        null
                    } else {
                        ReplicationEventPayload.Ack(messageId, peer)
                    }
                }
                "failed" -> {
                    val reason = root.optString("reason", "unknown error")
                    ReplicationEventPayload.Failed(messageId, reason)
                }
                "retry" -> {
                    val peer = root.optString("peer")
                    if (peer.isNullOrEmpty()) {
                        Log.w(TAG, "Retry event missing peer: $payload")
                        null
                    } else {
                        ReplicationEventPayload.Retry(messageId, peer)
                    }
                }
                else -> {
                    Log.w(TAG, "Unknown replication event type: $payload")
                    null
                }
            }
        } catch (error: JSONException) {
            Log.w(TAG, "Failed to parse replication event payload: $payload", error)
            null
        }
    }

    fun buildStubEnvelope(body: String = "hello from android"): String {
        val envelope = JSONObject()
        envelope.put("message_id", UUID.randomUUID().toString())
        envelope.put("conversation_id", UUID.randomUUID().toString())
        envelope.put("sender_fingerprint", "android-stub-fpr")
        envelope.put("sender_device", UUID.randomUUID().toString())
        envelope.put("created_ms", System.currentTimeMillis())

        val payload = JSONObject()
        payload.put("nonce", "AAAAAAAAAAAAAAAAAAAAAAAAAA==")
        payload.put("ciphertext", android.util.Base64.encodeToString(body.toByteArray(), android.util.Base64.NO_WRAP))
        envelope.put("payload", payload)

        envelope.put("signature", android.util.Base64.encodeToString("stub-signature".toByteArray(), android.util.Base64.NO_WRAP))
        return envelope.toString()
    }
}
