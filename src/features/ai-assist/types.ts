export interface AiMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  sqlBlock?: string;
  createdAt: number;
}
