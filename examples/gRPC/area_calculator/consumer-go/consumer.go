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

func GetSquareArea(address string) (float32, error) {
	// Set up a connection to the server.
	conn, err := grpc.Dial(address, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("did not connect: %v", err)
		return 0, err
	}
	defer conn.Close()

	c := ac.NewCalculatorClient(conn)

	fmt.Println("Sending calculate square request")
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	r, err := c.Calculate(ctx, &ac.ShapeMessage{ Shape: &ac.ShapeMessage_Square{ Square: &ac.Square { EdgeLength: 3 } } })
	if err != nil {
		return 0, err
	}
	return r.GetValue(), nil
}

func main() {
	flag.Parse()
	area, err := GetSquareArea(*addr)
	if err != nil {
		log.Fatalf("could not calculate length: %v", err)
	} else {
		fmt.Printf("Area: %f", area)
	}
}
