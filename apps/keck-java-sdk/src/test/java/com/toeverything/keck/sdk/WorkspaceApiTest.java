package com.toeverything.keck.sdk;

import com.google.gson.JsonElement;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;

import static org.junit.Assert.*;

public class WorkspaceApiTest {

    private MockWebServer server;
    private WorkspaceApi api;

    @Before
    public void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        KeckConfig config = new KeckConfig(server.url("/").toString());
        KeckHttpClient client = new KeckHttpClient(config);
        api = new WorkspaceApi(client);
    }

    @After
    public void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    public void testCreateWorkspace() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"workspace\":\"test-ws\"}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = api.createWorkspace("test-ws");
        assertNotNull(result);

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        assertEquals("/api/block/test-ws", request.getPath());
    }

    @Test
    public void testGetWorkspace() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"id\":\"test-ws\",\"blocks\":[]}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = api.getWorkspace("test-ws");
        assertNotNull(result);
        assertTrue(result.isJsonObject());

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws", request.getPath());
    }

    @Test
    public void testDeleteWorkspace() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(204));

        api.deleteWorkspace("test-ws");

        RecordedRequest request = server.takeRequest();
        assertEquals("DELETE", request.getMethod());
        assertEquals("/api/block/test-ws", request.getPath());
    }

    @Test
    public void testExportWorkspace() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"data\":\"export-data\"}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = api.exportWorkspace("test-ws");
        assertNotNull(result);

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/export", request.getPath());
    }

    @Test
    public void testGetHistory() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"history\":[]}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = api.getHistory("test-ws");
        assertNotNull(result);

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/history", request.getPath());
    }

    @Test
    public void testInitWorkspace() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"workspace\":\"test-ws\"}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = api.initWorkspace("test-ws");
        assertNotNull(result);

        RecordedRequest request = server.takeRequest();
        assertEquals("POST", request.getMethod());
        assertEquals("/api/block/test-ws/init", request.getPath());
    }
}
