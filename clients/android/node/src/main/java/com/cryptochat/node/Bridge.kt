package com.cryptochat.node

import androidx.annotation.Keep

@Keep
interface EventCallback {
    fun onEvent(payload: String)
}

object Bridge {
    init {
        System.loadLibrary("cryptochat_node")
    }

    external fun startNode(configDir: String)
    external fun stopNode()
    external fun registerCallback(callback: EventCallback)
    external fun unregisterCallback()
    external fun publishEnvelope(envelopeJson: String): Boolean
}
