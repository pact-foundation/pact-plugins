import type * as grpc from '@grpc/grpc-js';
import type { MessageTypeDefinition } from '@grpc/proto-loader';

import type { CalculatorClient as _area_calculator_CalculatorClient, CalculatorDefinition as _area_calculator_CalculatorDefinition } from './area_calculator/Calculator';

type SubtypeConstructor<Constructor extends new (...args: any) => any, Subtype> = {
  new(...args: ConstructorParameters<Constructor>): Subtype;
};

export interface ProtoGrpcType {
  area_calculator: {
    AreaRequest: MessageTypeDefinition
    AreaResponse: MessageTypeDefinition
    Calculator: SubtypeConstructor<typeof grpc.Client, _area_calculator_CalculatorClient> & { service: _area_calculator_CalculatorDefinition }
    Circle: MessageTypeDefinition
    Parallelogram: MessageTypeDefinition
    Rectangle: MessageTypeDefinition
    ShapeMessage: MessageTypeDefinition
    Square: MessageTypeDefinition
    Triangle: MessageTypeDefinition
  }
}

