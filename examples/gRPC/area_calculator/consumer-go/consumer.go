package main

import (
	ac "area_calculator/consumer/io.pact/area_calculator"
	"context"
	"flag"
	"fmt"
	"log"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
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

	log.Println("Sending calculate square request")
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	r, err := c.CalculateOne(ctx, &ac.ShapeMessage{Shape: &ac.ShapeMessage_Square{Square: &ac.Square{EdgeLength: 3}}})
	if err != nil {
		return 0, err
	}
	return r.GetValue()[0], nil
}

func GetRectangleArea(address string) (float32, error) {
	// Set up a connection to the server.
	conn, err := grpc.Dial(address, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("did not connect: %v", err)
		return 0, err
	}
	defer conn.Close()

	c := ac.NewCalculatorClient(conn)

	log.Println("Sending calculate rectangle request")
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()

	shape := ac.ShapeMessage{Shape: &ac.ShapeMessage_Rectangle{Rectangle: &ac.Rectangle{Length: 3, Width: 4}}}
	r, err := c.CalculateOne(ctx, &shape)
	if err != nil {
		return 0, err
	}
	return r.GetValue()[0], nil
}

func GetRectangleAndSquareArea(address string) ([]float32, error) {
	// Set up a connection to the server.
	conn, err := grpc.Dial(address, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("did not connect: %v", err)
		return nil, err
	}
	defer conn.Close()

	c := ac.NewCalculatorClient(conn)

	log.Println("Sending calculate rectangle and square request")
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()

	rectangle := ac.ShapeMessage{Shape: &ac.ShapeMessage_Rectangle{Rectangle: &ac.Rectangle{Length: 3, Width: 4}}}
	square := ac.ShapeMessage{Shape: &ac.ShapeMessage_Square{Square: &ac.Square{EdgeLength: 3}}}
	r, err := c.CalculateMulti(ctx, &ac.AreaRequest{Shapes: []*ac.ShapeMessage{&rectangle, &square}})
	if err != nil {
		return nil, err
	}
	return r.GetValue(), nil
}

func main() {
	flag.Parse()
	area, err := GetRectangleAndSquareArea(*addr)
	if err != nil {
		log.Fatalf("could not calculate lengths: %v", err)
	} else {
		fmt.Printf("Areas: %f", area)
	}
}
