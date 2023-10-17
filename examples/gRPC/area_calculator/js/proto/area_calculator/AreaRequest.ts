// Original file: proto/area_calculator.proto

import type { ShapeMessage as _area_calculator_ShapeMessage, ShapeMessage__Output as _area_calculator_ShapeMessage__Output } from '../area_calculator/ShapeMessage';

export interface AreaRequest {
  'shapes'?: (_area_calculator_ShapeMessage)[];
}

export interface AreaRequest__Output {
  'shapes': (_area_calculator_ShapeMessage__Output)[];
}
