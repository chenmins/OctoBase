package com.toeverything.keck.sdk;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;

import static org.junit.Assert.*;

public class YArrayTest {

    private MockWebServer server;
    private YArray yArray;

    @Before
    public void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        KeckConfig config = new KeckConfig(server.url("/").toString());
        KeckHttpClient client = new KeckHttpClient(config);
        yArray = new YArray(client, "test-ws", "tags");
    }

    @After
    public void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    public void testGetAllElements() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[\"tag1\",\"tag2\",\"tag3\"]")
                .setHeader("Content-Type", "application/json"));

        assertEquals(3, yArray.size());

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/array/tags", request.getPath());
    }

    @Test
    public void testGetElementAtIndex() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("\"tag1\"")
                .setHeader("Content-Type", "application/json"));

        Object value = yArray.get(0);
        assertEquals("tag1", value);

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/array/tags/0", request.getPath());
    }

    @Test
    public void testGetNumericElement() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("42")
                .setHeader("Content-Type", "application/json"));

        Object value = yArray.get(0);
        assertEquals(42, value);
    }

    @Test(expected = IndexOutOfBoundsException.class)
    public void testGetInvalidIndex() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(404));

        yArray.get(99);
    }

    @Test(expected = IndexOutOfBoundsException.class)
    public void testGetNegativeIndex() throws Exception {
        yArray.get(-1);
    }

    @Test
    public void testPush() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[\"tag1\",\"new-tag\"]")
                .setHeader("Content-Type", "application/json"));

        boolean added = yArray.add("new-tag");
        assertTrue(added);

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        assertEquals("/api/block/test-ws/array/tags", request.getPath());
        String body = request.getBody().readUtf8();
        assertTrue(body.contains("\"action\":\"push\""));
        assertTrue(body.contains("\"value\":\"new-tag\""));
    }

    @Test
    public void testInsertAtIndex() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[\"first-tag\",\"tag1\"]")
                .setHeader("Content-Type", "application/json"));

        yArray.add(0, "first-tag");

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        assertEquals("/api/block/test-ws/array/tags", request.getPath());
        String body = request.getBody().readUtf8();
        assertTrue(body.contains("\"action\":\"insert\""));
        assertTrue(body.contains("\"index\":0"));
        assertTrue(body.contains("\"value\":\"first-tag\""));
    }

    @Test(expected = IndexOutOfBoundsException.class)
    public void testInsertNegativeIndex() throws Exception {
        yArray.add(-1, "test");
    }

    @Test
    public void testRemoveAtIndex() throws Exception {
        // GET current value
        server.enqueue(new MockResponse()
                .setBody("\"tag1\"")
                .setHeader("Content-Type", "application/json"));
        // DELETE
        server.enqueue(new MockResponse().setResponseCode(204));

        Object removed = yArray.remove(0);
        assertEquals("tag1", removed);

        RecordedRequest getReq = server.takeRequest();
        assertEquals("GET", getReq.getMethod());

        RecordedRequest deleteReq = server.takeRequest();
        assertEquals("DELETE", deleteReq.getMethod());
        assertEquals("/api/block/test-ws/array/tags/0", deleteReq.getPath());
    }

    @Test
    public void testSet() throws Exception {
        // GET for remove
        server.enqueue(new MockResponse()
                .setBody("\"old-value\"")
                .setHeader("Content-Type", "application/json"));
        // DELETE for remove
        server.enqueue(new MockResponse().setResponseCode(204));
        // POST for insert
        server.enqueue(new MockResponse()
                .setBody("[\"new-value\"]")
                .setHeader("Content-Type", "application/json"));

        Object previous = yArray.set(0, "new-value");
        assertEquals("old-value", previous);
    }

    @Test
    public void testIsEmpty() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[]")
                .setHeader("Content-Type", "application/json"));

        assertTrue(yArray.isEmpty());
    }

    @Test
    public void testPushAlias() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[\"v\"]")
                .setHeader("Content-Type", "application/json"));

        yArray.push("v");

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        String body = request.getBody().readUtf8();
        assertTrue(body.contains("\"action\":\"push\""));
    }

    @Test
    public void testInsertAlias() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[\"v\"]")
                .setHeader("Content-Type", "application/json"));

        yArray.insert(0, "v");

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        String body = request.getBody().readUtf8();
        assertTrue(body.contains("\"action\":\"insert\""));
    }

    @Test
    public void testGetWorkspaceAndName() {
        assertEquals("test-ws", yArray.getWorkspace());
        assertEquals("tags", yArray.getName());
    }

    @Test
    public void testEmpty404() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(404));

        assertEquals(0, yArray.size());
    }
}
