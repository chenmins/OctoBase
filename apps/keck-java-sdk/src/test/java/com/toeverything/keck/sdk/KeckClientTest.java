package com.toeverything.keck.sdk;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import java.util.Map;
import java.util.Set;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;

import static org.junit.Assert.*;

/**
 * Tests for the KeckClient facade class.
 */
public class KeckClientTest {

    private MockWebServer server;
    private KeckClient keckClient;

    @Before
    public void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        KeckConfig config = new KeckConfig(server.url("/").toString());
        keckClient = new KeckClient(config);
    }

    @After
    public void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    public void testDefaultConstructor() {
        KeckClient client = new KeckClient();
        assertNotNull(client.getConfig());
        assertNotNull(client.workspaces());
        assertNotNull(client.doc());
    }

    @Test
    public void testWorkspaceApi() {
        assertNotNull(keckClient.workspaces());
    }

    @Test
    public void testDocApi() {
        assertNotNull(keckClient.doc());
    }

    @Test
    public void testMapCreation() {
        YMap map = keckClient.map("ws1", "settings");
        assertNotNull(map);
        assertEquals("ws1", map.getWorkspace());
        assertEquals("settings", map.getName());
    }

    @Test
    public void testArrayCreation() {
        YArray array = keckClient.array("ws1", "tags");
        assertNotNull(array);
        assertEquals("ws1", array.getWorkspace());
        assertEquals("tags", array.getName());
    }

    @Test
    public void testSubscribeCreation() {
        SseSubscription sub = keckClient.subscribe("ws1", event -> {});
        assertNotNull(sub);
        assertFalse(sub.isRunning());
    }

    @Test
    public void testMapInterface() throws Exception {
        // Create workspace
        server.enqueue(new MockResponse()
                .setBody("{\"workspace\":\"test-ws\"}")
                .setHeader("Content-Type", "application/json"));
        keckClient.workspaces().createWorkspace("test-ws");

        // Use YMap as java.util.Map
        YMap settings = keckClient.map("test-ws", "settings");

        // put: GET existing (404) + POST
        server.enqueue(new MockResponse().setResponseCode(404));
        server.enqueue(new MockResponse()
                .setBody("{\"theme\":\"dark\"}")
                .setHeader("Content-Type", "application/json"));
        settings.put("theme", "dark");

        // get
        server.enqueue(new MockResponse()
                .setBody("\"dark\"")
                .setHeader("Content-Type", "application/json"));
        assertEquals("dark", settings.get("theme"));

        // size
        server.enqueue(new MockResponse()
                .setBody("{\"theme\":\"dark\"}")
                .setHeader("Content-Type", "application/json"));
        assertEquals(1, settings.size());
    }

    @Test
    public void testArrayInterface() throws Exception {
        // Create workspace
        server.enqueue(new MockResponse()
                .setBody("{\"workspace\":\"test-ws\"}")
                .setHeader("Content-Type", "application/json"));
        keckClient.workspaces().createWorkspace("test-ws");

        // Use YArray as java.util.List
        YArray tags = keckClient.array("test-ws", "tags");

        // add (push)
        server.enqueue(new MockResponse()
                .setBody("[\"tag1\"]")
                .setHeader("Content-Type", "application/json"));
        tags.add("tag1");

        // get
        server.enqueue(new MockResponse()
                .setBody("\"tag1\"")
                .setHeader("Content-Type", "application/json"));
        assertEquals("tag1", tags.get(0));

        // size
        server.enqueue(new MockResponse()
                .setBody("[\"tag1\"]")
                .setHeader("Content-Type", "application/json"));
        assertEquals(1, tags.size());
    }

    @Test
    public void testFullWorkflow() throws Exception {
        // 1. Create workspace
        server.enqueue(new MockResponse()
                .setBody("{\"workspace\":\"demo\"}")
                .setHeader("Content-Type", "application/json"));
        keckClient.workspaces().createWorkspace("demo");

        // 2. Use map
        YMap map = keckClient.map("demo", "config");
        server.enqueue(new MockResponse().setResponseCode(404)); // get prev
        server.enqueue(new MockResponse()
                .setBody("{\"key1\":\"val1\"}")
                .setHeader("Content-Type", "application/json"));
        map.put("key1", "val1");

        // 3. Use array
        YArray arr = keckClient.array("demo", "items");
        server.enqueue(new MockResponse()
                .setBody("[\"item1\"]")
                .setHeader("Content-Type", "application/json"));
        arr.add("item1");

        // 4. Delete workspace
        server.enqueue(new MockResponse().setResponseCode(204));
        keckClient.workspaces().deleteWorkspace("demo");
    }
}
