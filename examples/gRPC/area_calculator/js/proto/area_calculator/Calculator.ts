// Original file: proto/area_calculator.proto

import type * as grpc from '@grpc/grpc-js'
import type { MethodDefinition } from '@grpc/proto-loader'
import type { AreaRequest as _area_calculator_AreaRequest, AreaRequest__Output as _area_calculator_AreaRequest__Output } from '../area_calculator/AreaRequest';
import type { AreaResponse as _area_calculator_AreaResponse, AreaResponse__Output as _area_calculator_AreaResponse__Output } from '../area_calculator/AreaResponse';
import type { ShapeMessage as _area_calculator_ShapeMessage, ShapeMessage__Output as _area_calculator_ShapeMessage__Output } from '../area_calculator/ShapeMessage';

export interface CalculatorClient extends grpc.Client {
  calculateMulti(argument: _area_calculator_AreaRequest, metadata: grpc.Metadata, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, metadata: grpc.Metadata, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, metadata: grpc.Metadata, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, metadata: grpc.Metadata, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateMulti(argument: _area_calculator_AreaRequest, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  
  calculateOne(argument: _area_calculator_ShapeMessage, metadata: grpc.Metadata, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, metadata: grpc.Metadata, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, metadata: grpc.Metadata, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, metadata: grpc.Metadata, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, options: grpc.CallOptions, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  calculateOne(argument: _area_calculator_ShapeMessage, callback: grpc.requestCallback<_area_calculator_AreaResponse__Output>): grpc.ClientUnaryCall;
  
}

export interface CalculatorHandlers extends grpc.UntypedServiceImplementation {
  calculateMulti: grpc.handleUnaryCall<_area_calculator_AreaRequest__Output, _area_calculator_AreaResponse>;
  
  calculateOne: grpc.handleUnaryCall<_area_calculator_ShapeMessage__Output, _area_calculator_AreaResponse>;
  
}

export interface CalculatorDefinition extends grpc.ServiceDefinition {
  calculateMulti: MethodDefinition<_area_calculator_AreaRequest, _area_calculator_AreaResponse, _area_calculator_AreaRequest__Output, _area_calculator_AreaResponse__Output>
  calculateOne: MethodDefinition<_area_calculator_ShapeMessage, _area_calculator_AreaResponse, _area_calculator_ShapeMessage__Output, _area_calculator_AreaResponse__Output>
}
