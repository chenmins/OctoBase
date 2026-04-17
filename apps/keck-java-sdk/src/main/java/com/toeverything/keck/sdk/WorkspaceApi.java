package com.toeverything.keck.sdk;

import com.google.gson.JsonElement;

/**
 * API client for Workspace CRUD operations.
 *
 * <ul>
 *   <li>GET    /api/block/{workspace}          - Get a workspace</li>
 *   <li>POST   /api/block/{workspace}          - Create a workspace</li>
 *   <li>DELETE  /api/block/{workspace}          - Delete a workspace</li>
 *   <li>GET    /api/block/{workspace}/export    - Export a workspace</li>
 *   <li>GET    /api/block/{workspace}/history   - Get workspace history</li>
 *   <li>POST   /api/block/{workspace}/init      - Init a workspace</li>
 * </ul>
 */
public class WorkspaceApi {

    private final KeckHttpClient client;

    public WorkspaceApi(KeckHttpClient client) {
        this.client = client;
    }

    /**
     * Get an existing workspace by id.
     *
     * @param workspace workspace id
     * @return JSON response representing the workspace
     */
    public JsonElement getWorkspace(String workspace) {
        return client.get("api/block/" + workspace);
    }

    /**
     * Create a workspace by id.
     *
     * @param workspace workspace id
     * @return JSON response
     */
    public JsonElement createWorkspace(String workspace) {
        return client.postEmpty("api/block/" + workspace);
    }

    /**
     * Delete an existing workspace by id.
     *
     * @param workspace workspace id
     */
    public void deleteWorkspace(String workspace) {
        client.delete("api/block/" + workspace);
    }

    /**
     * Export a workspace by id.
     *
     * @param workspace workspace id
     * @return JSON response containing the export data
     */
    public JsonElement exportWorkspace(String workspace) {
        return client.get("api/block/" + workspace + "/export");
    }

    /**
     * Get the history of a workspace.
     *
     * @param workspace workspace id
     * @return JSON response containing the history
     */
    public JsonElement getHistory(String workspace) {
        return client.get("api/block/" + workspace + "/history");
    }

    /**
     * Init a workspace by id.
     *
     * @param workspace workspace id
     * @return JSON response
     */
    public JsonElement initWorkspace(String workspace) {
        return client.postEmpty("api/block/" + workspace + "/init");
    }
}
