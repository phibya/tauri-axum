import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '../api/client'
import { Conversation, Message } from '../types/api/chat'

interface Branch {
  id: string
  conversation_id: string
  created_at: string
}

interface ChatState {
  // Current conversation state
  currentConversation: Conversation | null
  currentMessages: Message[]
  currentBranches: Branch[]
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
  createConversation: (
    assistantId: string,
    modelProviderId: string,
    modelId: string,
  ) => Promise<string>
  loadConversation: (conversationId: string) => Promise<void>
  sendMessage: (
    content: string,
    assistantId: string,
    modelProviderId: string,
    modelId: string,
  ) => Promise<void>
  editMessage: (messageId: string, newContent: string) => Promise<void>
  loadMessageBranches: (messageId: string) => Promise<Branch[]>
  switchBranch: (conversationId: string, branchId: string) => Promise<void>
  stopStreaming: () => void
  clearError: () => void
  reset: () => void
}

export const useChatStore = create<ChatState>()(
  subscribeWithSelector((set, get) => ({
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

    createConversation: async (
      assistantId: string,
      modelProviderId: string,
      modelId: string,
    ) => {
      try {
        set({ loading: true, error: null })

        const response = await ApiClient.Chat.createConversation({
          title: 'New Conversation',
          assistant_id: assistantId,
          model_provider_id: modelProviderId,
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

    loadConversation: async (conversationId: string) => {
      try {
        set({ loading: true, error: null })

        const response = await ApiClient.Chat.getConversation({
          conversation_id: conversationId,
        })

        set({
          currentConversation: response.conversation,
          currentMessages: response.messages,
          activeBranchId: response.conversation.active_branch_id,
          loading: false,
        })
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
      modelProviderId: string,
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
          branch_id: activeBranchId,
          is_active_branch: true,
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
          branch_id: activeBranchId,
          is_active_branch: true,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
          edit_count: 0,
          originated_from_id: crypto.randomUUID(),
        }

        set(state => ({
          currentMessages: [...state.currentMessages, assistantMessage],
        }))

        // Send message with streaming
        const response = await ApiClient.Chat.sendMessage({
          conversation_id: currentConversation.id,
          content,
          model_provider_id: modelProviderId,
          model_id: modelId,
        })

        // Update with actual response
        set(state => ({
          currentMessages: state.currentMessages.map(msg =>
            msg.id === assistantMessage.id
              ? { ...msg, ...response.assistant_message }
              : msg.id === userMessage.id
                ? { ...msg, ...response.user_message }
                : msg,
          ),
          sending: false,
          isStreaming: false,
          streamingMessage: '',
        }))
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
        set({ sending: true, error: null })

        const response = await ApiClient.Chat.editMessage({
          message_id: messageId,
          content: newContent,
        })

        // Response is just a Message, not containing messages/new_branch_id
        set(state => ({
          currentMessages: state.currentMessages.map(msg =>
            msg.id === messageId ? response : msg,
          ),
          sending: false,
        }))
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to edit message',
          sending: false,
        })
        throw error
      }
    },

    loadMessageBranches: async (messageId: string) => {
      try {
        set({ loadingBranches: true, error: null })

        const branches = await ApiClient.Chat.getMessageBranches({
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
            error instanceof Error ? error.message : 'Failed to load branches',
          loadingBranches: false,
        })
        throw error
      }
    },

    switchBranch: async (_conversationId: string, branchId: string) => {
      try {
        set({ loading: true, error: null })

        const response = await ApiClient.Chat.switchBranch({
          message_id: branchId,
        })

        // If response doesn't contain messages, keep current messages
        set({
          currentMessages: (response as any).messages || get().currentMessages,
          activeBranchId: branchId,
          loading: false,
        })
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to switch branch',
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
  })),
)
