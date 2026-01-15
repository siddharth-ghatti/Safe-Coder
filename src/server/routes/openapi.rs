//! OpenAPI specification endpoint

use axum::Json;

/// GET /api/openapi.json - Get OpenAPI specification
pub async fn openapi_spec() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Safe-Coder API",
            "description": "HTTP API for safe-coder desktop app integration",
            "version": env!("CARGO_PKG_VERSION")
        },
        "servers": [
            {
                "url": "http://localhost:9876",
                "description": "Local development server"
            }
        ],
        "paths": {
            "/api/health": {
                "get": {
                    "summary": "Health check",
                    "operationId": "healthCheck",
                    "responses": {
                        "200": {
                            "description": "Server is healthy",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/HealthResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/config": {
                "get": {
                    "summary": "Get configuration",
                    "operationId": "getConfig",
                    "responses": {
                        "200": {
                            "description": "Current configuration",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ConfigResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/sessions": {
                "get": {
                    "summary": "List sessions",
                    "operationId": "listSessions",
                    "responses": {
                        "200": {
                            "description": "List of active sessions"
                        }
                    }
                },
                "post": {
                    "summary": "Create session",
                    "operationId": "createSession",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/CreateSessionRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Session created"
                        }
                    }
                }
            },
            "/api/sessions/{id}": {
                "get": {
                    "summary": "Get session",
                    "operationId": "getSession",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": { "description": "Session details" },
                        "404": { "description": "Session not found" }
                    }
                },
                "delete": {
                    "summary": "Delete session",
                    "operationId": "deleteSession",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "204": { "description": "Session deleted" },
                        "404": { "description": "Session not found" }
                    }
                }
            },
            "/api/sessions/{id}/messages": {
                "get": {
                    "summary": "Get messages",
                    "operationId": "getMessages",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": { "description": "Message history" }
                    }
                },
                "post": {
                    "summary": "Send message",
                    "operationId": "sendMessage",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/SendMessageRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Message sent, subscribe to events for updates" }
                    }
                }
            },
            "/api/sessions/{id}/events": {
                "get": {
                    "summary": "Subscribe to session events (SSE)",
                    "operationId": "sessionEvents",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Server-sent events stream",
                            "content": {
                                "text/event-stream": {}
                            }
                        }
                    }
                }
            },
            "/api/sessions/{id}/changes": {
                "get": {
                    "summary": "Get file changes",
                    "operationId": "getSessionChanges",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": { "description": "File changes for session" }
                    }
                }
            },
            "/api/sessions/{id}/pty": {
                "get": {
                    "summary": "PTY WebSocket connection",
                    "operationId": "ptyWebSocket",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "101": { "description": "WebSocket upgrade" }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "HealthResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string" },
                        "version": { "type": "string" }
                    }
                },
                "ConfigResponse": {
                    "type": "object",
                    "properties": {
                        "provider": { "type": "string" },
                        "model": { "type": "string" },
                        "mode": { "type": "string" }
                    }
                },
                "CreateSessionRequest": {
                    "type": "object",
                    "required": ["project_path"],
                    "properties": {
                        "project_path": { "type": "string" },
                        "mode": { "type": "string" }
                    }
                },
                "SendMessageRequest": {
                    "type": "object",
                    "required": ["content"],
                    "properties": {
                        "content": { "type": "string" },
                        "attachments": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/Attachment" }
                        }
                    }
                },
                "Attachment": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    }
                },
                "ServerEvent": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/ThinkingEvent" },
                        { "$ref": "#/components/schemas/ToolStartEvent" },
                        { "$ref": "#/components/schemas/TextChunkEvent" },
                        { "$ref": "#/components/schemas/FileDiffEvent" }
                    ]
                },
                "ThinkingEvent": {
                    "type": "object",
                    "properties": {
                        "type": { "const": "Thinking" },
                        "message": { "type": "string" }
                    }
                },
                "ToolStartEvent": {
                    "type": "object",
                    "properties": {
                        "type": { "const": "ToolStart" },
                        "name": { "type": "string" },
                        "description": { "type": "string" }
                    }
                },
                "TextChunkEvent": {
                    "type": "object",
                    "properties": {
                        "type": { "const": "TextChunk" },
                        "text": { "type": "string" }
                    }
                },
                "FileDiffEvent": {
                    "type": "object",
                    "properties": {
                        "type": { "const": "FileDiff" },
                        "path": { "type": "string" },
                        "additions": { "type": "integer" },
                        "deletions": { "type": "integer" },
                        "diff": { "type": "string" }
                    }
                }
            }
        }
    }))
}
