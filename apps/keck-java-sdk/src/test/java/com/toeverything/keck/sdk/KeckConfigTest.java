package com.toeverything.keck.sdk;

import org.junit.Test;

import static org.junit.Assert.*;

public class KeckConfigTest {

    @Test
    public void testConstructorWithTrailingSlash() {
        KeckConfig config = new KeckConfig("http://localhost:3000/");
        assertEquals("http://localhost:3000/", config.getEndpoint());
    }

    @Test
    public void testConstructorWithoutTrailingSlash() {
        KeckConfig config = new KeckConfig("http://localhost:3000");
        assertEquals("http://localhost:3000/", config.getEndpoint());
    }

    @Test
    public void testBuildUrl() {
        KeckConfig config = new KeckConfig("http://localhost:3000/");
        assertEquals("http://localhost:3000/api/block/ws1", config.buildUrl("api/block/ws1"));
    }

    @Test
    public void testBuildUrlWithLeadingSlash() {
        KeckConfig config = new KeckConfig("http://localhost:3000/");
        assertEquals("http://localhost:3000/api/block/ws1", config.buildUrl("/api/block/ws1"));
    }

    @Test
    public void testLoadDefault() {
        KeckConfig config = KeckConfig.loadDefault();
        assertNotNull(config);
        assertNotNull(config.getEndpoint());
        assertTrue(config.getEndpoint().endsWith("/"));
    }
}
