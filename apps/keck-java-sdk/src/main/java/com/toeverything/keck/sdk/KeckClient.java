package com.toeverything.keck.sdk;

import okhttp3.OkHttpClient;

/**
 * Main entry point for the Keck Java SDK.
 * <p>
 * Provides convenient access to all API areas: Workspace management,
 * YMap, YArray, Doc, and SSE subscriptions.
 *
 * <h3>Usage Example</h3>
 * <pre>
 * // Using default config (loads keck-sdk.properties)
 * KeckClient keck = new KeckClient();
 *
 * // Create a workspace
 * keck.workspaces().createWorkspace("my-workspace");
 *
 * // Use a YMap (java.util.Map interface)
 * YMap settings = keck.map("my-workspace", "settings");
 * settings.put("theme", "dark");
 * settings.put("fontSize", 14);
 * String theme = (String) settings.get("theme");
 *
 * // Use a YArray (java.util.List interface)
 * YArray tags = keck.array("my-workspace", "tags");
 * tags.add("tag1");
 * tags.add("tag2");
 * Object first = tags.get(0);
 *
 * // List doc keys
 * List&lt;String&gt; keys = keck.doc().listKeys("my-workspace");
 *
 * // Subscribe to SSE events
 * SseSubscription sub = keck.subscribe("my-workspace", event -&gt; {
 *     System.out.println(event.getData());
 * });
 * // sub.start() blocks; call sub.close() from another thread
 *
 * // Clean up
 * keck.workspaces().deleteWorkspace("my-workspace");
 * </pre>
 */
public class KeckClient {

    private final KeckHttpClient httpClient;
    private final WorkspaceApi workspaceApi;
    private final DocApi docApi;

    /**
     * Create a KeckClient with default config loaded from classpath.
     */
    public KeckClient() {
        this(KeckConfig.loadDefault());
    }

    /**
     * Create a KeckClient with the given config.
     */
    public KeckClient(KeckConfig config) {
        this.httpClient = new KeckHttpClient(config);
        this.workspaceApi = new WorkspaceApi(httpClient);
        this.docApi = new DocApi(httpClient);
    }

    /**
     * Create a KeckClient with a custom OkHttpClient.
     */
    public KeckClient(KeckConfig config, OkHttpClient okHttpClient) {
        this.httpClient = new KeckHttpClient(config, okHttpClient);
        this.workspaceApi = new WorkspaceApi(httpClient);
        this.docApi = new DocApi(httpClient);
    }

    /**
     * @return the workspace API for CRUD operations on workspaces
     */
    public WorkspaceApi workspaces() {
        return workspaceApi;
    }

    /**
     * @return the doc API for document-level operations
     */
    public DocApi doc() {
        return docApi;
    }

    /**
     * Create a YMap (java.util.Map) bound to a remote Y.Map.
     *
     * @param workspace workspace id
     * @param name      Y.Map name
     * @return a Map implementation backed by the Keck API
     */
    public YMap map(String workspace, String name) {
        return new YMap(httpClient, workspace, name);
    }

    /**
     * Create a YArray (java.util.List) bound to a remote Y.Array.
     *
     * @param workspace workspace id
     * @param name      Y.Array name
     * @return a List implementation backed by the Keck API
     */
    public YArray array(String workspace, String name) {
        return new YArray(httpClient, workspace, name);
    }

    /**
     * Create an SSE subscription for real-time doc changes.
     *
     * @param workspace workspace id
     * @param listener  event listener
     * @return subscription object; call start() to begin listening
     */
    public SseSubscription subscribe(String workspace, SseSubscription.SseEventListener listener) {
        return new SseSubscription(httpClient, workspace, listener);
    }

    /**
     * @return the underlying HTTP client
     */
    public KeckHttpClient getHttpClient() {
        return httpClient;
    }

    /**
     * @return the configuration
     */
    public KeckConfig getConfig() {
        return httpClient.getConfig();
    }
}
