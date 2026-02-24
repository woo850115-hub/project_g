export type AttributeValueType = 'number' | 'string' | 'boolean' | 'range' | 'select' | 'tags';

export interface SelectOption {
  value: string;
  label: string;
}

export interface AttributeSchema {
  id: string;
  label: string;
  description: string;
  category: string;
  value_type: AttributeValueType;
  default: unknown;
  applies_to: string[];
  options: SelectOption[];
}
