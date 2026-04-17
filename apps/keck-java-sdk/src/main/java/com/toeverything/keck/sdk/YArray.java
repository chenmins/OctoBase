package com.toeverything.keck.sdk;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;

import java.util.AbstractList;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A {@link java.util.List} implementation backed by the Keck YArray API.
 * <p>
 * Each mutating operation is immediately synchronized with the remote server.
 * Read operations fetch the current state from the server.
 *
 * <h3>API Endpoints Used</h3>
 * <ul>
 *   <li>GET    /api/block/{workspace}/array/{name}             - Get all elements</li>
 *   <li>GET    /api/block/{workspace}/array/{name}/{index}      - Get element at index</li>
 *   <li>POST   /api/block/{workspace}/array/{name}             - Push / insert elements</li>
 *   <li>DELETE  /api/block/{workspace}/array/{name}/{index}      - Delete element at index</li>
 * </ul>
 */
public class YArray extends AbstractList<Object> {

    private final KeckHttpClient client;
    private final String workspace;
    private final String name;
    private final String basePath;

    /**
     * Create a YArray that is bound to a remote array on the Keck server.
     *
     * @param client    the HTTP client
     * @param workspace workspace id
     * @param name      Y.Array name
     */
    public YArray(KeckHttpClient client, String workspace, String name) {
        this.client = client;
        this.workspace = workspace;
        this.name = name;
        this.basePath = "api/block/" + workspace + "/array/" + name;
    }

    public String getWorkspace() {
        return workspace;
    }

    public String getName() {
        return name;
    }

    // ── Read Operations ──────────────────────────────────────────────────

    /**
     * Fetch all elements from the server.
     */
    private List<Object> fetchAll() {
        JsonElement result = client.getOrNull(basePath);
        if (result == null || !result.isJsonArray()) {
            return Collections.emptyList();
        }
        JsonArray arr = result.getAsJsonArray();
        List<Object> list = new ArrayList<>(arr.size());
        for (JsonElement el : arr) {
            list.add(YMap.toJavaObject(el));
        }
        return list;
    }

    @Override
    public Object get(int index) {
        if (index < 0) {
            throw new IndexOutOfBoundsException("Index: " + index);
        }
        JsonElement result = client.getOrNull(basePath + "/" + index);
        if (result == null) {
            throw new IndexOutOfBoundsException("Index: " + index);
        }
        return YMap.toJavaObject(result);
    }

    @Override
    public int size() {
        return fetchAll().size();
    }

    @Override
    public boolean isEmpty() {
        return size() == 0;
    }

    // ── Write Operations ─────────────────────────────────────────────────

    /**
     * Append an element at the end of the array (push).
     */
    @Override
    public boolean add(Object element) {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("action", "push");
        body.put("value", element);
        client.post(basePath, body);
        return true;
    }

    /**
     * Insert an element at the given index.
     */
    @Override
    public void add(int index, Object element) {
        if (index < 0) {
            throw new IndexOutOfBoundsException("Index: " + index);
        }
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("action", "insert");
        body.put("index", index);
        body.put("value", element);
        client.post(basePath, body);
    }

    /**
     * Remove the element at the given index.
     */
    @Override
    public Object remove(int index) {
        if (index < 0) {
            throw new IndexOutOfBoundsException("Index: " + index);
        }
        Object previous = get(index);
        client.delete(basePath + "/" + index);
        return previous;
    }

    /**
     * Set is implemented as remove + insert.
     */
    @Override
    public Object set(int index, Object element) {
        Object previous = remove(index);
        add(index, element);
        return previous;
    }

    /**
     * Clear all elements by removing from the end one by one.
     */
    @Override
    public void clear() {
        int sz = size();
        // Remove from end to start to avoid index shifting issues
        for (int i = sz - 1; i >= 0; i--) {
            client.delete(basePath + "/" + i);
        }
    }

    /**
     * Push a value at the end (alias for add).
     */
    public void push(Object value) {
        add(value);
    }

    /**
     * Insert a value at the given index (alias for add(int, Object)).
     */
    public void insert(int index, Object value) {
        add(index, value);
    }
}
