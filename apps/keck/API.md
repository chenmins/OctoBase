# Keck API Documentation

Keck is the collaboration backend for OctoBase. This document covers the **YType API** endpoints for manipulating Y.js-compatible CRDT data structures (Maps, Arrays) and subscribing to real-time changes via SSE.

> **Base URL**: `http://localhost:3000` (default, configurable via `KECK_PORT` env var)

---

## Table of Contents

- [YMap Endpoints](#ymap-endpoints)
  - [Get all map entries](#get-all-map-entries)
  - [Get a specific map key](#get-a-specific-map-key)
  - [Set map key-value pairs](#set-map-key-value-pairs)
  - [Delete a map key](#delete-a-map-key)
- [YArray Endpoints](#yarray-endpoints)
  - [Get all array elements](#get-all-array-elements)
  - [Get element at index](#get-element-at-index)
  - [Push / Insert array elements](#push--insert-array-elements)
  - [Delete element at index](#delete-element-at-index)
- [Doc Endpoints](#doc-endpoints)
  - [List root-level type names](#list-root-level-type-names)
- [SSE Subscription](#sse-subscription)
  - [Subscribe to doc changes](#subscribe-to-doc-changes)

---

## YMap Endpoints

### Get all map entries

Retrieve all key-value pairs from a named Y.Map in the workspace.

| Field | Value |
|-------|-------|
| **Method** | `GET` |
| **Path** | `/api/block/{workspace}/map/{name}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Map name |

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | JSON object containing all map entries |
| `404 Not Found` | Workspace or map not found |

**Example**

```bash
curl -s http://localhost:3000/api/block/my-workspace/map/settings
```

---

### Get a specific map key

Retrieve the value of a specific key from a named Y.Map.

| Field | Value |
|-------|-------|
| **Method** | `GET` |
| **Path** | `/api/block/{workspace}/map/{name}/{key}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Map name |
| `key` | string | Map key to look up |

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | JSON value for the key |
| `404 Not Found` | Workspace, map, or key not found |

**Example**

```bash
curl -s http://localhost:3000/api/block/my-workspace/map/settings/theme
```

---

### Set map key-value pairs

Create or update one or more key-value pairs in a named Y.Map. The request body must be a JSON object; each field in the object will be set as a key in the map.

| Field | Value |
|-------|-------|
| **Method** | `POST` |
| **Path** | `/api/block/{workspace}/map/{name}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Map name |

**Request Body** — `application/json`

A JSON object whose keys and values will be set in the map.

```json
{
  "theme": "dark",
  "fontSize": 14,
  "showLineNumbers": true
}
```

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | Updated map returned as JSON |
| `400 Bad Request` | Request body is not a JSON object |
| `404 Not Found` | Workspace not found |
| `500 Internal Server Error` | Failed to set map entries |

**Example**

```bash
curl -s -X POST http://localhost:3000/api/block/my-workspace/map/settings \
  -H "Content-Type: application/json" \
  -d '{"theme":"dark","fontSize":14}'
```

---

### Delete a map key

Remove a key from a named Y.Map.

| Field | Value |
|-------|-------|
| **Method** | `DELETE` |
| **Path** | `/api/block/{workspace}/map/{name}/{key}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Map name |
| `key` | string | Map key to delete |

**Response**

| Status | Description |
|--------|-------------|
| `204 No Content` | Key deleted successfully |
| `404 Not Found` | Workspace or map not found |

**Example**

```bash
curl -s -X DELETE http://localhost:3000/api/block/my-workspace/map/settings/theme
```

---

## YArray Endpoints

### Get all array elements

Retrieve all elements from a named Y.Array in the workspace.

| Field | Value |
|-------|-------|
| **Method** | `GET` |
| **Path** | `/api/block/{workspace}/array/{name}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Array name |

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | JSON array of all elements |
| `404 Not Found` | Workspace or array not found |

**Example**

```bash
curl -s http://localhost:3000/api/block/my-workspace/array/tags
```

---

### Get element at index

Retrieve a single element at the given index from a named Y.Array.

| Field | Value |
|-------|-------|
| **Method** | `GET` |
| **Path** | `/api/block/{workspace}/array/{name}/{index}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Array name |
| `index` | integer | Zero-based array index |

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | JSON value of the element |
| `404 Not Found` | Workspace, array, or index not found |

**Example**

```bash
curl -s http://localhost:3000/api/block/my-workspace/array/tags/0
```

---

### Push / Insert array elements

Add elements to a named Y.Array. Two actions are supported:

- **push** — Append a value at the end of the array.
- **insert** — Insert a value at a specific index.

| Field | Value |
|-------|-------|
| **Method** | `POST` |
| **Path** | `/api/block/{workspace}/array/{name}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Array name |

**Request Body** — `application/json`

For **push**:

```json
{
  "action": "push",
  "value": "new-tag"
}
```

For **insert**:

```json
{
  "action": "insert",
  "index": 0,
  "value": "first-tag"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `action` | string | Yes | `"push"` or `"insert"` |
| `value` | any | Yes | Value to add to the array |
| `index` | integer | Only for `insert` | Zero-based position to insert at |

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | Updated array returned as JSON |
| `400 Bad Request` | Missing or invalid `action`, `value`, or `index` field |
| `404 Not Found` | Workspace not found |
| `500 Internal Server Error` | Failed to modify array |

**Example — Push**

```bash
curl -s -X POST http://localhost:3000/api/block/my-workspace/array/tags \
  -H "Content-Type: application/json" \
  -d '{"action":"push","value":"new-tag"}'
```

**Example — Insert at index 0**

```bash
curl -s -X POST http://localhost:3000/api/block/my-workspace/array/tags \
  -H "Content-Type: application/json" \
  -d '{"action":"insert","index":0,"value":"first-tag"}'
```

---

### Delete element at index

Remove the element at the given index from a named Y.Array.

| Field | Value |
|-------|-------|
| **Method** | `DELETE` |
| **Path** | `/api/block/{workspace}/array/{name}/{index}` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |
| `name` | string | Y.Array name |
| `index` | integer | Zero-based index of element to remove |

**Response**

| Status | Description |
|--------|-------------|
| `204 No Content` | Element removed successfully |
| `404 Not Found` | Workspace or array not found |
| `500 Internal Server Error` | Failed to remove element |

**Example**

```bash
curl -s -X DELETE http://localhost:3000/api/block/my-workspace/array/tags/0
```

---

## Doc Endpoints

### List root-level type names

List all root-level named types (maps, arrays, text, etc.) that exist in the workspace document.

| Field | Value |
|-------|-------|
| **Method** | `GET` |
| **Path** | `/api/block/{workspace}/doc/keys` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |

**Response**

| Status | Description |
|--------|-------------|
| `200 OK` | JSON array of root-level type name strings |
| `404 Not Found` | Workspace not found |

**Example**

```bash
curl -s http://localhost:3000/api/block/my-workspace/doc/keys
```

---

## SSE Subscription

### Subscribe to doc changes

Subscribe to real-time workspace document changes via **Server-Sent Events** (SSE). The server keeps the connection open and pushes events whenever the document is modified.

| Field | Value |
|-------|-------|
| **Method** | `GET` |
| **Path** | `/api/block/{workspace}/subscribe/sse` |

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | string | Workspace ID |

**Response**

The response is an SSE stream (`text/event-stream`). Each event has:

- **Event type**: `update`
- **Data**: JSON object with the following structure:

```json
{
  "update": [/* binary update as array of bytes */],
  "histories": [
    {
      "field_name": "optional-field-name",
      "parent": ["parent-id-1"],
      "content": "...",
      "action": "add"
    }
  ]
}
```

| Status | Description |
|--------|-------------|
| `200 OK` | SSE stream established |
| `404 Not Found` | Workspace not found |

**Example**

```bash
# This will keep the connection open and print events as they arrive.
# Press Ctrl+C to stop.
curl -s -N http://localhost:3000/api/block/my-workspace/subscribe/sse
```

---

## Quick Reference

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/block/{workspace}/map/{name}` | Get all map entries |
| `GET` | `/api/block/{workspace}/map/{name}/{key}` | Get a specific key value |
| `POST` | `/api/block/{workspace}/map/{name}` | Set key-value pairs (JSON object body) |
| `DELETE` | `/api/block/{workspace}/map/{name}/{key}` | Delete a map key |
| `GET` | `/api/block/{workspace}/array/{name}` | Get all array elements |
| `GET` | `/api/block/{workspace}/array/{name}/{index}` | Get element at index |
| `POST` | `/api/block/{workspace}/array/{name}` | Push/insert elements |
| `DELETE` | `/api/block/{workspace}/array/{name}/{index}` | Remove element at index |
| `GET` | `/api/block/{workspace}/doc/keys` | List root-level type names |
| `GET` | `/api/block/{workspace}/subscribe/sse` | SSE stream for real-time changes |
