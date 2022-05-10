package main

import (
	"context"
	"fmt"
	"flag"
	"log"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	ac "area_calculator/consumer/io.pact/area_calculator"
	"time"
)

var (
	addr = flag.String("addr", "localhost:8080", "the address to connect to")
)

func main() {
	flag.Parse()
	// Set up a connection to the server.
	conn, err := grpc.Dial(*addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("did not connect: %v", err)
	}
	defer conn.Close()

	c := ac.NewCalculatorClient(conn)

	fmt.Println("Sending calculate square request")
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	r, err := c.Calculate(ctx, &ac.ShapeMessage{ Shape: &ac.ShapeMessage_Square{ Square: &ac.Square { EdgeLength: 3 } } })
	if err != nil {
		log.Fatalf("could not calculate length: %v", err)
	}
	fmt.Printf("Area: %f", r.GetValue())
}
