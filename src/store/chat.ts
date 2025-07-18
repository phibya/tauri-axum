import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '../api/client'
import { Conversation, Message, MessageBranch } from '../types/api/chat'
import { useConversationsStore } from './conversations.ts'

export interface ChatState {
  // Current conversation state
  currentConversation: Conversation | null
  currentMessages: Message[]
  currentBranches: MessageBranch[]
  activeBranchId: string | null

  // Loading states
  loading: boolean
  sending: boolean
  loadingBranches: boolean

  // Error state
  error: string | null

  // Stream state
  streamingMessage: string
  isStreaming: boolean

  // Actions
  createConversation: (assistantId: string, modelId: string) => Promise<string>
  loadConversation: (
    conversationId: string,
    loadMessages: boolean,
  ) => Promise<void>
  sendMessage: (
    content: string,
    assistantId: string,
    modelId: string,
  ) => Promise<void>
  editMessage: (messageId: string, newContent: string) => Promise<void>
  loadMessageBranches: (messageId: string) => Promise<MessageBranch[]>
  switchBranch: (conversationId: string, branchId: string) => Promise<void>
  stopStreaming: () => void
  clearError: () => void
  reset: () => void
}

export const useChatStore = create<ChatState>()(
  subscribeWithSelector(
    (set, get): ChatState => ({
      // Initial state
      currentConversation: null,
      currentMessages: [],
      currentBranches: [],
      activeBranchId: null,
      loading: false,
      sending: false,
      loadingBranches: false,
      error: null,
      streamingMessage: '',
      isStreaming: false,

      createConversation: async (assistantId: string, modelId: string) => {
        try {
          set({ loading: true, error: null })

          const response = await ApiClient.Chat.createConversation({
            title: 'New Conversation', // This will be auto-generated by the backend
            assistant_id: assistantId,
            model_id: modelId,
          })

          set({
            currentConversation: response,
            currentMessages: [],
            activeBranchId: response.active_branch_id,
            loading: false,
          })

          return response.id
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to create conversation',
            loading: false,
          })
          throw error
        }
      },

      loadConversation: async (
        conversationId: string,
        loadMessages: boolean = true,
      ) => {
        try {
          set({ loading: true, error: null })

          // Get conversation info
          const conversation = await ApiClient.Chat.getConversation({
            conversation_id: conversationId,
          })

          useConversationsStore.setState(state => ({
            conversations: state.conversations.map(conv => {
              if (conv.id === conversationId) {
                return {
                  ...conv,
                  title: conversation.title || conv.title,
                }
              }
              return conv
            }),
          }))

          if (loadMessages) {
            // Load messages only if requested
            const messages = await ApiClient.Chat.getConversationMessages({
              conversation_id: conversationId,
              branch_id: conversation.active_branch_id,
            })

            set({
              currentConversation: conversation,
              currentMessages: messages,
              currentBranches: [],
              activeBranchId: conversation.active_branch_id,
              loading: false,
            })
          } else {
            set({
              currentConversation: conversation,
              currentBranches: [],
              activeBranchId: conversation.active_branch_id,
              loading: false,
            })
          }
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to load conversation',
            loading: false,
          })
          throw error
        }
      },

      sendMessage: async (
        content: string,
        _assistantId: string,
        modelId: string,
      ) => {
        const { currentConversation, activeBranchId } = get()
        if (!currentConversation || !activeBranchId) return

        try {
          set({
            sending: true,
            error: null,
            isStreaming: true,
            streamingMessage: '',
          })

          // Add user message immediately
          const { currentConversation, activeBranchId } = get()
          if (!currentConversation || !activeBranchId) return

          const userMessage: Message = {
            id: crypto.randomUUID(),
            conversation_id: currentConversation.id,
            content,
            role: 'user',
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
            edit_count: 0,
            originated_from_id: crypto.randomUUID(),
          }

          set(state => ({
            currentMessages: [...state.currentMessages, userMessage],
          }))

          // Create assistant message placeholder
          const assistantMessage: Message = {
            id: crypto.randomUUID(),
            conversation_id: currentConversation.id,
            content: '',
            role: 'assistant',
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
            edit_count: 0,
            originated_from_id: crypto.randomUUID(),
          }

          set(state => ({
            currentMessages: [...state.currentMessages, assistantMessage],
          }))

          // Send message with streaming
          await ApiClient.Chat.sendMessage(
            {
              conversation_id: currentConversation.id,
              content,
              model_id: modelId,
            },
            {
              onChunk(data: { delta: string }) {
                set(state => ({
                  streamingMessage: state.streamingMessage + data.delta,
                }))
              },
              onComplete(
                _data: Omit<Message, 'content'> & { message_id: string },
              ) {
                set(state => ({
                  isStreaming: false,
                  sending: false,
                  streamingMessage: '',
                  currentMessages: [
                    ...state.currentMessages,
                    {
                      ...assistantMessage,
                      content: state.streamingMessage,
                      updated_at: new Date().toISOString(),
                      id: _data.message_id,
                    },
                  ],
                }))
              },
              onError() {
                set({
                  error: 'Streaming failed',
                  sending: false,
                  isStreaming: false,
                  streamingMessage: '',
                })
              },
            },
          )
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to send message',
            sending: false,
            isStreaming: false,
            streamingMessage: '',
          })
          throw error
        }
      },

      editMessage: async (messageId: string, newContent: string) => {
        const { currentConversation } = get()
        if (!currentConversation) return

        try {
          set({
            sending: true,
            error: null,
            isStreaming: true,
            streamingMessage: '',
          })

          const currentMessage = get().currentMessages.find(
            msg => msg.id === messageId,
          )

          if (!currentMessage) {
            throw new Error('Message not found')
          }

          // Update the user message immediately with the new content
          set(state => {
            let currentMessages = state.currentMessages.filter(
              m =>
                new Date(m.created_at) <= new Date(currentMessage.created_at),
            )

            return {
              currentMessages: currentMessages.map(msg =>
                msg.id === messageId ? { ...msg, content: newContent } : msg,
              ),
            }
          })

          // Create assistant message placeholder for streaming
          const assistantMessage: Message = {
            id: 'streaming-temp',
            conversation_id: currentConversation.id,
            content: '',
            role: 'assistant',
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
            edit_count: 0,
            originated_from_id: messageId,
          }

          set(state => ({
            currentMessages: [...state.currentMessages, assistantMessage],
          }))

          // Use streaming edit endpoint
          await ApiClient.Chat.editMessageStream(
            {
              message_id: messageId,
              content: newContent,
            },
            {
              onChunk(data: { delta: string }) {
                set(state => ({
                  streamingMessage: state.streamingMessage + data.delta,
                }))
              },
              onComplete(
                data: Omit<Message, 'content'> & { message_id: string },
              ) {
                set(state => ({
                  isStreaming: false,
                  sending: false,
                  streamingMessage: '',
                  currentMessages: [
                    ...state.currentMessages.filter(
                      msg => msg.id !== 'streaming-temp',
                    ),
                    {
                      ...assistantMessage,
                      content: state.streamingMessage,
                      updated_at: new Date().toISOString(),
                      id: data.message_id,
                    },
                  ],
                }))
              },
              onError() {
                set({
                  error: 'Edit streaming failed',
                  sending: false,
                  isStreaming: false,
                  streamingMessage: '',
                  // Remove the streaming placeholder
                  currentMessages: get().currentMessages.filter(
                    msg => msg.id !== 'streaming-temp',
                  ),
                })
              },
            },
          )
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to edit message',
            sending: false,
            isStreaming: false,
            streamingMessage: '',
            // Remove the streaming placeholder on error
            currentMessages: get().currentMessages.filter(
              msg => msg.id !== 'streaming-temp',
            ),
          })
          throw error
        }
      },

      loadMessageBranches: async (messageId: string) => {
        try {
          set({ loadingBranches: true, error: null })

          let branches = await ApiClient.Chat.getMessageBranches({
            message_id: messageId,
          })

          set({
            currentBranches: branches,
            loadingBranches: false,
          })

          return branches
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to load branches',
            loadingBranches: false,
          })
          throw error
        }
      },

      switchBranch: async (conversationId: string, branchId: string) => {
        try {
          set({ error: null })

          await ApiClient.Chat.switchConversationBranch({
            conversation_id: conversationId,
            branch_id: branchId,
          })

          // After switching, reload the conversation and get messages for the new branch
          const conversation = await ApiClient.Chat.getConversation({
            conversation_id: conversationId,
          })

          const messages = await ApiClient.Chat.getConversationMessages({
            conversation_id: conversationId,
            branch_id: branchId,
          })

          set({
            currentConversation: conversation,
            currentMessages: messages,
            activeBranchId: branchId,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to switch branch',
            loading: false,
          })
          throw error
        }
      },

      stopStreaming: () => {
        set({ isStreaming: false, sending: false })
      },

      clearError: () => {
        set({ error: null })
      },

      reset: () => {
        set({
          currentConversation: null,
          currentMessages: [],
          currentBranches: [],
          activeBranchId: null,
          loading: false,
          sending: false,
          loadingBranches: false,
          error: null,
          streamingMessage: '',
          isStreaming: false,
        })
      },
    }),
  ),
)
