package main

import (
    "log"
    "net"
    "net/http"
    "time"

    "github.com/gin-gonic/gin"
    "google.golang.org/grpc"
    "github.com/segmentio/kafka-go"
)

type GatewayServer struct {
    kafkaWriter *kafka.Writer
}

func NewGatewayServer() *GatewayServer {
    writer := &kafka.Writer{
        Addr:     kafka.TCP("localhost:9092"),
        Topic:    "gateway.requests",
        Balancer: &kafka.LeastBytes{},
    }

    return &GatewayServer{
        kafkaWriter: writer,
    }
}

func (s *GatewayServer) handleHealthCheck(c *gin.Context) {
    c.JSON(http.StatusOK, gin.H{
        "status":    "healthy",
        "service":   "gateway",
        "timestamp": time.Now().UTC(),
    })
}

func (s *GatewayServer) handleMetrics(c *gin.Context) {
    c.JSON(http.StatusOK, gin.H{
        "requests_total": 0,
        "uptime_seconds": time.Now().Unix(),
    })
}

func main() {
    server := NewGatewayServer()
    
    // HTTP API
    r := gin.Default()
    r.GET("/health", server.handleHealthCheck)
    r.GET("/metrics", server.handleMetrics)
    
    // Start HTTP server
    go func() {
        log.Println("Gateway HTTP server starting on :8080")
        if err := r.Run(":8080"); err != nil {
            log.Fatal("Failed to start HTTP server:", err)
        }
    }()
    
    // gRPC server (placeholder)
    lis, err := net.Listen("tcp", ":50051")
    if err != nil {
        log.Fatalf("Failed to listen: %v", err)
    }
    
    grpcServer := grpc.NewServer()
    
    log.Println("Gateway gRPC server starting on :50051")
    if err := grpcServer.Serve(lis); err != nil {
        log.Fatalf("Failed to serve: %v", err)
    }
}