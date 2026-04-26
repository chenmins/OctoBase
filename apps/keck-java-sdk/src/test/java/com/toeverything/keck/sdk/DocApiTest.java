package com.toeverything.keck.sdk;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import java.util.List;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;

import static org.junit.Assert.*;

public class DocApiTest {

    private MockWebServer server;
    private DocApi docApi;

    @Before
    public void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        KeckConfig config = new KeckConfig(server.url("/").toString());
        KeckHttpClient client = new KeckHttpClient(config);
        docApi = new DocApi(client);
    }

    @After
    public void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    public void testListKeys() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[\"settings\",\"tags\",\"notes\"]")
                .setHeader("Content-Type", "application/json"));

        List<String> keys = docApi.listKeys("test-ws");
        assertEquals(3, keys.size());
        assertEquals("settings", keys.get(0));
        assertEquals("tags", keys.get(1));
        assertEquals("notes", keys.get(2));

        RecordedRequest request = server.takeRequest();
        assertEquals("GET", request.getMethod());
        assertEquals("/api/block/test-ws/doc/keys", request.getPath());
    }

    @Test
    public void testListKeysEmpty() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("[]")
                .setHeader("Content-Type", "application/json"));

        List<String> keys = docApi.listKeys("test-ws");
        assertTrue(keys.isEmpty());
    }
}
