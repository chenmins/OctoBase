package com.toeverything.keck.sdk;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;

import java.util.List;

/**
 * API client for Doc-level endpoints.
 */
public class DocApi {

    private final KeckHttpClient client;

    public DocApi(KeckHttpClient client) {
        this.client = client;
    }

    /**
     * List all root-level type names (maps, arrays, text, etc.) in the workspace document.
     *
     * @param workspace workspace id
     * @return list of root-level type name strings
     */
    public List<String> listKeys(String workspace) {
        JsonElement result = client.get("api/block/" + workspace + "/doc/keys");
        Gson gson = client.getGson();
        if (result != null && result.isJsonArray()) {
            JsonArray arr = result.getAsJsonArray();
            java.util.List<String> keys = new java.util.ArrayList<>();
            for (JsonElement el : arr) {
                keys.add(el.getAsString());
            }
            return keys;
        }
        return java.util.Collections.emptyList();
    }
}
