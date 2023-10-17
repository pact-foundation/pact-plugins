// Original file: proto/area_calculator.proto


export interface Triangle {
  'edgeA'?: (number | string);
  'edgeB'?: (number | string);
  'edgeC'?: (number | string);
}

export interface Triangle__Output {
  'edgeA': (number);
  'edgeB': (number);
  'edgeC': (number);
}
