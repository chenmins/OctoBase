package com.toeverything.keck.sdk;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import java.util.HashMap;
import java.util.Map;
import java.util.Set;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;

import static org.junit.Assert.*;

public class YMapTest {

    private MockWebServer server;
    private YMap yMap;

    @Before
    public void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        KeckConfig config = new KeckConfig(server.url("/").toString());
        KeckHttpClient client = new KeckHttpClient(config);
        yMap = new YMap(client, "test-ws", "settings");
    }

    @After
    public void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    public void testGetAllEntries() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"theme\":\"dark\",\"fontSize\":14}")
                .setHeader("Content-Type", "application/json"));

        Set<Map.Entry<String, Object>> entries = yMap.entrySet();
        assertEquals(2, entries.size());

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/map/settings", request.getPath());
    }

    @Test
    public void testGetSpecificKey() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("\"dark\"")
                .setHeader("Content-Type", "application/json"));

        Object value = yMap.get("theme");
        assertEquals("dark", value);

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/map/settings/theme", request.getPath());
    }

    @Test
    public void testGetNumericValue() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("14")
                .setHeader("Content-Type", "application/json"));

        Object value = yMap.get("fontSize");
        assertEquals(14, value);
    }

    @Test
    public void testGetBooleanValue() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("true")
                .setHeader("Content-Type", "application/json"));

        Object value = yMap.get("showLineNumbers");
        assertEquals(true, value);
    }

    @Test
    public void testGetMissingKey() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(404));

        Object value = yMap.get("nonexistent");
        assertNull(value);
    }

    @Test
    public void testPutKeyValue() throws Exception {
        // First call: get previous value (404 = not found)
        server.enqueue(new MockResponse().setResponseCode(404));
        // Second call: POST to set value
        server.enqueue(new MockResponse()
                .setBody("{\"theme\":\"dark\"}")
                .setHeader("Content-Type", "application/json"));

        Object previous = yMap.put("theme", "dark");
        assertNull(previous);

        // Verify GET was issued first
        RecordedRequest getReq = server.takeRequest();
        assertEquals("GET", getReq.getMethod());
        assertTrue(getReq.getPath().contains("/theme"));

        // Verify POST was issued
        RecordedRequest postReq = server.takeRequest();
        assertEquals("POST", postReq.getMethod());
        assertEquals("/api/block/test-ws/map/settings", postReq.getPath());
        assertTrue(postReq.getBody().readUtf8().contains("\"theme\""));
    }

    @Test
    public void testPutAll() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"theme\":\"dark\",\"fontSize\":14}")
                .setHeader("Content-Type", "application/json"));

        Map<String, Object> values = new HashMap<>();
        values.put("theme", "dark");
        values.put("fontSize", 14);
        yMap.putAll(values);

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        assertEquals("/api/block/test-ws/map/settings", request.getPath());
    }

    @Test
    public void testRemoveKey() throws Exception {
        // GET to retrieve current value
        server.enqueue(new MockResponse()
                .setBody("\"dark\"")
                .setHeader("Content-Type", "application/json"));
        // DELETE
        server.enqueue(new MockResponse().setResponseCode(204));

        Object previous = yMap.remove("theme");
        assertEquals("dark", previous);

        RecordedRequest getReq = server.takeRequest();
        assertEquals("GET", getReq.getMethod());

        RecordedRequest deleteReq = server.takeRequest();
        assertEquals("DELETE", deleteReq.getMethod());
        assertEquals("/api/block/test-ws/map/settings/theme", deleteReq.getPath());
    }

    @Test
    public void testContainsKey() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("\"dark\"")
                .setHeader("Content-Type", "application/json"));

        assertTrue(yMap.containsKey("theme"));
    }

    @Test
    public void testContainsKeyMissing() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(404));

        assertFalse(yMap.containsKey("nonexistent"));
    }

    @Test
    public void testSize() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"a\":1,\"b\":2,\"c\":3}")
                .setHeader("Content-Type", "application/json"));

        assertEquals(3, yMap.size());
    }

    @Test
    public void testIsEmpty() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{}")
                .setHeader("Content-Type", "application/json"));

        assertTrue(yMap.isEmpty());
    }

    @Test
    public void testKeySet() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"a\":1,\"b\":2}")
                .setHeader("Content-Type", "application/json"));

        Set<String> keys = yMap.keySet();
        assertEquals(2, keys.size());
        assertTrue(keys.contains("a"));
        assertTrue(keys.contains("b"));
    }

    @Test
    public void testGetWorkspaceAndName() {
        assertEquals("test-ws", yMap.getWorkspace());
        assertEquals("settings", yMap.getName());
    }

    @Test
    public void testEmptyMap404() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(404));

        assertEquals(0, yMap.size());
    }
}
