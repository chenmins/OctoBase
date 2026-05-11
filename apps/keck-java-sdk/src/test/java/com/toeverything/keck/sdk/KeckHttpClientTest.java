package com.toeverything.keck.sdk;

import com.google.gson.JsonElement;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;

import static org.junit.Assert.*;

public class KeckHttpClientTest {

    private MockWebServer server;
    private KeckHttpClient client;

    @Before
    public void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        KeckConfig config = new KeckConfig(server.url("/").toString());
        client = new KeckHttpClient(config);
    }

    @After
    public void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    public void testGet() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"key\":\"value\"}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = client.get("test/path");
        assertNotNull(result);
        assertTrue(result.isJsonObject());
        assertEquals("value", result.getAsJsonObject().get("key").getAsString());
    }

    @Test
    public void testGetOrNull404() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(404));

        JsonElement result = client.getOrNull("test/path");
        assertNull(result);
    }

    @Test
    public void testPost() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"result\":\"ok\"}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = client.post("test/path", java.util.Collections.singletonMap("key", "value"));
        assertNotNull(result);
    }

    @Test
    public void testPostEmpty() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"created\":true}")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = client.postEmpty("test/path");
        assertNotNull(result);
    }

    @Test
    public void testDelete() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(204));

        client.delete("test/path");
        // No exception means success
    }

    @Test(expected = KeckException.class)
    public void testGetError500() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(500));

        client.get("test/path");
    }

    @Test(expected = KeckException.class)
    public void testDeleteError500() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(500));

        client.delete("test/path");
    }

    @Test
    public void testGetRaw() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("raw-text-response"));

        String result = client.getRaw("test/path");
        assertEquals("raw-text-response", result);
    }

    @Test
    public void testGetEmptyBody() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("")
                .setHeader("Content-Type", "application/json"));

        JsonElement result = client.getOrNull("test/path");
        assertNull(result);
    }
}
