package main

import (
	"errors"
	"log"
	"math"
	"net"

	context "context"

	"google.golang.org/grpc"

	ac "area_calculator/provider/io.pact/area_calculator"
)

type calculatorServer struct {
	ac.UnimplementedCalculatorServer
}

func CalculateArea(shape *ac.ShapeMessage) (float32, error) {
	switch shape.Shape.(type) {
	case *ac.ShapeMessage_Square:
		square := shape.GetSquare()
		return square.EdgeLength * square.EdgeLength, nil
	case *ac.ShapeMessage_Rectangle:
		rectangle := shape.GetRectangle()
		return rectangle.Length * rectangle.Width, nil
	case *ac.ShapeMessage_Circle:
		circle := shape.GetCircle()
		return math.Pi * circle.Radius * circle.Radius, nil
	case *ac.ShapeMessage_Triangle:
		triangle := shape.GetTriangle()
		p := (triangle.EdgeA + triangle.EdgeB + triangle.EdgeC) / 2.0
		return float32(math.Sqrt(float64(p * (p - triangle.EdgeA) * (p - triangle.EdgeB) * (p - triangle.EdgeC)))), nil
	case *ac.ShapeMessage_Parallelogram:
		parallelogram := shape.GetParallelogram()
		return parallelogram.BaseLength * parallelogram.Height, nil
	default:
		return 0, errors.New("not a valid shape")
	}
}

func (calc *calculatorServer) CalculateOne(ctx context.Context, req *ac.ShapeMessage) (*ac.AreaResponse, error) {
	var areas []float32

	log.Println("Calculating the area for one value", req)
	area, err := CalculateArea(req)
	if err != nil {
		return nil, err
	}
	areas = append(areas, area)

	return &ac.AreaResponse{Value: areas}, nil
}

func (calc *calculatorServer) CalculateMulti(ctx context.Context, req *ac.AreaRequest) (*ac.AreaResponse, error) {
	var areas []float32

	log.Println("Calculating the area for multiple values", req)
	for _, shape := range req.Shapes {
		area, err := CalculateArea(shape)
		if err != nil {
			return nil, err
		}
		areas = append(areas, area)
	}

	return &ac.AreaResponse{Value: areas}, nil
}

func NewServer() *calculatorServer {
	s := &calculatorServer{}
	return s
}

func main() {
	lis, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}

	log.Println("Server started", lis.Addr())

	var opts []grpc.ServerOption
	grpcServer := grpc.NewServer(opts...)
	ac.RegisterCalculatorServer(grpcServer, NewServer())
	grpcServer.Serve(lis)
}
