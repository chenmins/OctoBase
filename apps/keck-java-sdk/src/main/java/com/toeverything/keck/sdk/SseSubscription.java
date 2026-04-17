package com.toeverything.keck.sdk;

import java.io.BufferedReader;
import java.io.Closeable;
import java.io.IOException;
import java.io.InputStreamReader;

import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;

/**
 * SSE (Server-Sent Events) subscription for real-time workspace document changes.
 * <p>
 * Usage:
 * <pre>
 * SseSubscription sub = new SseSubscription(client, "my-workspace", event -&gt; {
 *     System.out.println("Event: " + event.getData());
 * });
 * sub.start(); // blocks the calling thread
 * // Call sub.close() from another thread to stop
 * </pre>
 */
public class SseSubscription implements Closeable {

    /**
     * Listener for SSE events.
     */
    public interface SseEventListener {
        void onEvent(SseEvent event);
    }

    /**
     * Represents a single SSE event.
     */
    public static class SseEvent {
        private final String eventType;
        private final String data;

        public SseEvent(String eventType, String data) {
            this.eventType = eventType;
            this.data = data;
        }

        public String getEventType() {
            return eventType;
        }

        public String getData() {
            return data;
        }

        @Override
        public String toString() {
            return "SseEvent{type='" + eventType + "', data='" + data + "'}";
        }
    }

    private final KeckHttpClient client;
    private final String workspace;
    private final SseEventListener listener;
    private volatile boolean running;
    private volatile Response activeResponse;

    public SseSubscription(KeckHttpClient client, String workspace, SseEventListener listener) {
        this.client = client;
        this.workspace = workspace;
        this.listener = listener;
        this.running = false;
    }

    /**
     * Start listening for SSE events. This method blocks the calling thread.
     * Call {@link #close()} from another thread to stop.
     */
    public void start() {
        running = true;
        OkHttpClient httpClient = client.getHttpClient();
        String url = client.getConfig().buildUrl("api/block/" + workspace + "/subscribe/sse");

        Request request = new Request.Builder()
                .url(url)
                .header("Accept", "text/event-stream")
                .get()
                .build();

        try {
            activeResponse = httpClient.newCall(request).execute();
            if (!activeResponse.isSuccessful()) {
                throw new KeckException("SSE subscribe failed: " + activeResponse.code(), activeResponse.code());
            }
            if (activeResponse.body() == null) {
                return;
            }
            try (BufferedReader reader = new BufferedReader(
                    new InputStreamReader(activeResponse.body().byteStream(), "UTF-8"))) {
                String eventType = null;
                StringBuilder dataBuilder = new StringBuilder();

                String line;
                while (running && (line = reader.readLine()) != null) {
                    if (line.isEmpty()) {
                        // Empty line means end of event
                        if (dataBuilder.length() > 0) {
                            String data = dataBuilder.toString();
                            if (data.endsWith("\n")) {
                                data = data.substring(0, data.length() - 1);
                            }
                            listener.onEvent(new SseEvent(
                                    eventType != null ? eventType : "message",
                                    data
                            ));
                        }
                        eventType = null;
                        dataBuilder.setLength(0);
                    } else if (line.startsWith("event:")) {
                        eventType = line.substring(6).trim();
                    } else if (line.startsWith("data:")) {
                        if (dataBuilder.length() > 0) {
                            dataBuilder.append("\n");
                        }
                        dataBuilder.append(line.substring(5).trim());
                    }
                    // Ignore comments (lines starting with :) and other fields
                }
            }
        } catch (IOException e) {
            if (running) {
                throw new KeckException("SSE connection error", e);
            }
            // If not running, the close() was called, which is expected
        } finally {
            running = false;
            if (activeResponse != null) {
                activeResponse.close();
            }
        }
    }

    /**
     * @return true if the subscription is currently active
     */
    public boolean isRunning() {
        return running;
    }

    /**
     * Stop the SSE subscription and close the connection.
     */
    @Override
    public void close() {
        running = false;
        if (activeResponse != null) {
            activeResponse.close();
        }
    }
}
