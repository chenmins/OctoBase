package com.toeverything.keck.sdk;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;

import java.util.AbstractMap;
import java.util.AbstractSet;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * A {@link java.util.Map} implementation backed by the Keck YMap API.
 * <p>
 * Each mutating operation is immediately synchronized with the remote server.
 * Read operations fetch the current state from the server.
 * <p>
 * Supports {@code String} keys and {@code Object} values (which are serialized
 * via Gson when writing and returned as Gson {@link JsonElement}s or Java
 * primitives when reading).
 *
 * <h3>API Endpoints Used</h3>
 * <ul>
 *   <li>GET    /api/block/{workspace}/map/{name}          - Get all entries</li>
 *   <li>GET    /api/block/{workspace}/map/{name}/{key}     - Get a key</li>
 *   <li>POST   /api/block/{workspace}/map/{name}          - Set key-value pairs</li>
 *   <li>DELETE  /api/block/{workspace}/map/{name}/{key}     - Delete a key</li>
 * </ul>
 */
public class YMap extends AbstractMap<String, Object> {

    private final KeckHttpClient client;
    private final String workspace;
    private final String name;
    private final String basePath;

    /**
     * Create a YMap that is bound to a remote map on the Keck server.
     *
     * @param client    the HTTP client
     * @param workspace workspace id
     * @param name      Y.Map name
     */
    public YMap(KeckHttpClient client, String workspace, String name) {
        this.client = client;
        this.workspace = workspace;
        this.name = name;
        this.basePath = "api/block/" + workspace + "/map/" + name;
    }

    public String getWorkspace() {
        return workspace;
    }

    public String getName() {
        return name;
    }

    // ── Read Operations ──────────────────────────────────────────────────

    /**
     * Fetch all map entries from the server and return the JSON as a Map.
     */
    private Map<String, Object> fetchAll() {
        JsonElement result = client.getOrNull(basePath);
        if (result == null || !result.isJsonObject()) {
            return Collections.emptyMap();
        }
        JsonObject obj = result.getAsJsonObject();
        Map<String, Object> map = new LinkedHashMap<>();
        for (Map.Entry<String, JsonElement> entry : obj.entrySet()) {
            map.put(entry.getKey(), toJavaObject(entry.getValue()));
        }
        return map;
    }

    @Override
    public Set<Entry<String, Object>> entrySet() {
        Map<String, Object> snapshot = fetchAll();
        Set<Entry<String, Object>> entries = new LinkedHashSet<>();
        for (Entry<String, Object> e : snapshot.entrySet()) {
            entries.add(new SimpleImmutableEntry<>(e.getKey(), e.getValue()));
        }
        return new AbstractSet<Entry<String, Object>>() {
            @Override
            public Iterator<Entry<String, Object>> iterator() {
                return entries.iterator();
            }

            @Override
            public int size() {
                return entries.size();
            }
        };
    }

    @Override
    public Object get(Object key) {
        if (!(key instanceof String)) {
            return null;
        }
        JsonElement result = client.getOrNull(basePath + "/" + key);
        return result != null ? toJavaObject(result) : null;
    }

    @Override
    public boolean containsKey(Object key) {
        if (!(key instanceof String)) {
            return false;
        }
        return client.getOrNull(basePath + "/" + key) != null;
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
     * Set a single key-value pair. The value will be JSON-serialized.
     */
    @Override
    public Object put(String key, Object value) {
        Object previous = get(key);
        Map<String, Object> body = new LinkedHashMap<>();
        body.put(key, value);
        client.post(basePath, body);
        return previous;
    }

    /**
     * Set multiple key-value pairs at once.
     */
    @Override
    public void putAll(Map<? extends String, ?> m) {
        if (m.isEmpty()) {
            return;
        }
        client.post(basePath, m);
    }

    /**
     * Remove a key from the map.
     */
    @Override
    public Object remove(Object key) {
        if (!(key instanceof String)) {
            return null;
        }
        Object previous = get(key);
        if (previous != null) {
            client.delete(basePath + "/" + key);
        }
        return previous;
    }

    // ── Convenience methods ──────────────────────────────────────────────

    @Override
    public Set<String> keySet() {
        return fetchAll().keySet();
    }

    @Override
    public Collection<Object> values() {
        return fetchAll().values();
    }

    @Override
    public void clear() {
        Set<String> keys = keySet();
        for (String key : keys) {
            client.delete(basePath + "/" + key);
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /**
     * Convert a JsonElement to a Java-friendly object.
     * <p>
     * Primitives are converted to their Java equivalents (String, Boolean, Number).
     * JSON objects are recursively converted to {@code Map<String, Object>}.
     * JSON arrays are recursively converted to {@code List<Object>}.
     */
    static Object toJavaObject(JsonElement element) {
        if (element == null || element instanceof JsonNull) {
            return null;
        }
        if (element.isJsonPrimitive()) {
            if (element.getAsJsonPrimitive().isBoolean()) {
                return element.getAsBoolean();
            } else if (element.getAsJsonPrimitive().isNumber()) {
                Number num = element.getAsNumber();
                // Return integer types when possible
                if (num.doubleValue() == num.longValue()) {
                    long l = num.longValue();
                    if (l >= Integer.MIN_VALUE && l <= Integer.MAX_VALUE) {
                        return (int) l;
                    }
                    return l;
                }
                return num.doubleValue();
            } else {
                return element.getAsString();
            }
        }
        if (element.isJsonObject()) {
            JsonObject obj = element.getAsJsonObject();
            Map<String, Object> map = new LinkedHashMap<>();
            for (Map.Entry<String, JsonElement> entry : obj.entrySet()) {
                map.put(entry.getKey(), toJavaObject(entry.getValue()));
            }
            return map;
        }
        if (element.isJsonArray()) {
            JsonArray arr = element.getAsJsonArray();
            List<Object> list = new ArrayList<>(arr.size());
            for (JsonElement el : arr) {
                list.add(toJavaObject(el));
            }
            return list;
        }
        // Fallback: return the element as-is
        return element;
    }
}
