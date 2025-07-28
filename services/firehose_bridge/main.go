package main

import (
    "context"
    "encoding/json"
    "log"
    "os"
    "time"
    "fmt"

    "github.com/segmentio/kafka-go"
)

// Simplified block structure for demo
type Block struct {
    Number    string    `json:"number"`
    Hash      string    `json:"hash"`
    Timestamp time.Time `json:"timestamp"`
    TxCount   int       `json:"tx_count"`
}

// Mock firehose client for demonstration
type MockFirehoseClient struct {
    blockNumber int64
}

func (m *MockFirehoseClient) GetBlocks(ctx context.Context) (<-chan Block, error) {
    blockChan := make(chan Block)
    
    go func() {
        defer close(blockChan)
        ticker := time.NewTicker(12 * time.Second) // Ethereum block time
        defer ticker.Stop()
        
        for {
            select {
            case <-ctx.Done():
                return
            case <-ticker.C:
                m.blockNumber++
                block := Block{
                    Number:    fmt.Sprintf("%d", m.blockNumber),
                    Hash:      fmt.Sprintf("0x%064d", m.blockNumber),
                    Timestamp: time.Now(),
                    TxCount:   int(m.blockNumber % 200), // Mock transaction count
                }
                
                select {
                case blockChan <- block:
                case <-ctx.Done():
                    return
                }
            }
        }
    }()
    
    return blockChan, nil
}

func main() {
    log.Println("Starting Firehose Bridge...")
    
    firehoseAddr := os.Getenv("FIREHOSE_GRPC")
    if firehoseAddr == "" {
        firehoseAddr = "firehose-ethereum:13042"
    }
    
    kafkaBrokers := os.Getenv("KAFKA_BROKERS")
    if kafkaBrokers == "" {
        kafkaBrokers = "redpanda:9092"
    }
    
    topic := os.Getenv("KAFKA_TOPIC")
    if topic == "" {
        topic = "onchain.events"
    }

    // Kafka writer configuration
    writer := &kafka.Writer{
        Addr:         kafka.TCP(kafkaBrokers),
        Topic:        topic,
        Balancer:     &kafka.LeastBytes{},
        BatchSize:    100,
        BatchTimeout: 10 * time.Millisecond,
        Compression:  kafka.Snappy,
    }
    defer writer.Close()

    log.Printf("Connected to Kafka at %s, topic: %s", kafkaBrokers, topic)

    // For demo purposes, use mock client
    // In production, this would connect to actual Firehose
    client := &MockFirehoseClient{blockNumber: 18000000} // Start from recent block
    
    ctx := context.Background()
    
    // Alternative: Real gRPC connection (commented out for demo)
    /*
    conn, err := grpc.Dial(firehoseAddr, grpc.WithTransportCredentials(insecure.NewCredentials()))
    if err != nil {
        log.Printf("Warning: Could not connect to firehose at %s: %v", firehoseAddr, err)
        log.Println("Using mock data instead...")
    } else {
        defer conn.Close()
        log.Printf("Connected to Firehose at %s", firehoseAddr)
    }
    */

    blockChan, err := client.GetBlocks(ctx)
    if err != nil {
        log.Fatalf("Failed to get block stream: %v", err)
    }

    log.Println("Firehose bridge is running, processing blocks...")
    
    messageCount := 0
    for block := range blockChan {
        // Create enhanced message with block data
        blockData := map[string]interface{}{
            "block_number": block.Number,
            "block_hash":   block.Hash,
            "timestamp":    block.Timestamp,
            "tx_count":     block.TxCount,
            "source":       "ethereum",
            "bridge_time":  time.Now(),
        }
        
        blockJSON, err := json.Marshal(blockData)
        if err != nil {
            log.Printf("Error marshaling block data: %v", err)
            continue
        }
        
        msg := kafka.Message{
            Key:   []byte(fmt.Sprintf("eth_block_%s", block.Number)),
            Value: blockJSON,
            Time:  time.Now(),
            Headers: []kafka.Header{
                {Key: "source", Value: []byte("firehose-bridge")},
                {Key: "block_number", Value: []byte(block.Number)},
            },
        }
        
        if err := writer.WriteMessages(ctx, msg); err != nil {
            log.Printf("Kafka write error: %v", err)
        } else {
            messageCount++
            if messageCount%10 == 0 {
                log.Printf("Processed %d blocks, latest: %s", messageCount, block.Number)
            }
        }
    }
    
    log.Println("Firehose bridge shutting down...")
} 