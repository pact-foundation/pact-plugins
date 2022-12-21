package main

import (
	"context"
	"flag"
	"log"
	ac "test_enums/consumer/io.pact/test_enum"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

var (
	addr = flag.String("addr", "localhost:8080", "the address to connect to")
)

func GetFeature(address string) (ac.Feature, error) {
	// Set up a connection to the server.
	conn, err := grpc.Dial(address, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("did not connect: %v", err)
		return ac.Feature{}, err
	}
	defer conn.Close()

	c := ac.NewTestClient(conn)

	log.Println("Sending get feature request")
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	r, err := c.GetFeature2(ctx, &ac.Point{Latitude: 1000, Longitude: 1000})
	if err != nil {
		return ac.Feature{}, err
	}
	return *r, nil
}

func main() {
	flag.Parse()
	_, err := GetFeature(*addr)
	if err != nil {
		log.Fatalf("could not get feature: %v", err)
	} else {
		log.Print("Got Feature")
	}
}
