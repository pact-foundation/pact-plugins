// Original file: proto/area_calculator.proto

import type { Square as _area_calculator_Square, Square__Output as _area_calculator_Square__Output } from '../area_calculator/Square';
import type { Rectangle as _area_calculator_Rectangle, Rectangle__Output as _area_calculator_Rectangle__Output } from '../area_calculator/Rectangle';
import type { Circle as _area_calculator_Circle, Circle__Output as _area_calculator_Circle__Output } from '../area_calculator/Circle';
import type { Triangle as _area_calculator_Triangle, Triangle__Output as _area_calculator_Triangle__Output } from '../area_calculator/Triangle';
import type { Parallelogram as _area_calculator_Parallelogram, Parallelogram__Output as _area_calculator_Parallelogram__Output } from '../area_calculator/Parallelogram';

export interface ShapeMessage {
  'square'?: (_area_calculator_Square | null);
  'rectangle'?: (_area_calculator_Rectangle | null);
  'circle'?: (_area_calculator_Circle | null);
  'triangle'?: (_area_calculator_Triangle | null);
  'parallelogram'?: (_area_calculator_Parallelogram | null);
  'shape'?: "square"|"rectangle"|"circle"|"triangle"|"parallelogram";
}

export interface ShapeMessage__Output {
  'square'?: (_area_calculator_Square__Output | null);
  'rectangle'?: (_area_calculator_Rectangle__Output | null);
  'circle'?: (_area_calculator_Circle__Output | null);
  'triangle'?: (_area_calculator_Triangle__Output | null);
  'parallelogram'?: (_area_calculator_Parallelogram__Output | null);
  'shape': "square"|"rectangle"|"circle"|"triangle"|"parallelogram";
}
