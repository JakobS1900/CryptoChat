package com.cryptochat.app

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Spacer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.Button
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.tooling.preview.Preview
import com.cryptochat.node.NodeClient
import com.cryptochat.node.ReplicationEventPayload

class MainActivity : ComponentActivity() {
    private val latestEvent = mutableStateOf("No replication events yet")

    private val replicationListener: (ReplicationEventPayload) -> Unit = { event ->
        val message = when (event) {
            is ReplicationEventPayload.Queued -> "Message ${event.messageId} queued for replication"
            is ReplicationEventPayload.Ack -> "Message ${event.messageId} acknowledged by ${event.peer}"
            is ReplicationEventPayload.Failed -> "Message ${event.messageId} failed: ${event.reason}"
            is ReplicationEventPayload.Retry -> "Retrying message ${event.messageId} with ${event.peer}"
        }
        runOnUiThread {
            latestEvent.value = message
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        try {
            NodeClient.start(filesDir.absolutePath)
        } catch (t: Throwable) {
            Log.e("CryptoChat", "Failed to start native node", t)
        }

        setContent {
            val event by latestEvent
            CryptoChatApp(
                latestEvent = event,
                onSendTestEnvelope = { sendStubEnvelope() }
            )
        }
    }

    override fun onStart() {
        super.onStart()
        try {
            NodeClient.addReplicationListener(replicationListener)
        } catch (t: Throwable) {
            Log.e("CryptoChat", "Failed to add replication listener", t)
        }
    }

    override fun onStop() {
        super.onStop()
        NodeClient.removeReplicationListener(replicationListener)
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            NodeClient.stop()
        } catch (t: Throwable) {
            Log.e("CryptoChat", "Failed to stop native node", t)
        }
    }

    private fun sendStubEnvelope() {
        val envelope = NodeClient.buildStubEnvelope()
        if (!NodeClient.publishEnvelope(envelope)) {
            Log.w("CryptoChat", "Stub envelope publish request failed")
        }
    }
}

@Composable
fun CryptoChatApp(
    latestEvent: String,
    onSendTestEnvelope: () -> Unit,
) {
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Column(
                modifier = Modifier
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center
            ) {
                Text(text = "CryptoChat prototype node is running.")
                Spacer(modifier = Modifier.height(16.dp))
                Text(text = latestEvent, modifier = Modifier.align(Alignment.CenterHorizontally))
                Spacer(modifier = Modifier.height(24.dp))
                Button(onClick = onSendTestEnvelope) {
                    Text(text = "Send Stub Envelope")
                }
            }
        }
    }
}

@Preview
@Composable
fun PreviewCryptoChatApp() {
    CryptoChatApp(
        latestEvent = "No replication events yet",
        onSendTestEnvelope = {}
    )
}
