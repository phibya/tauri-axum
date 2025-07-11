import React, { useEffect, useState } from 'react'
import {
  App,
  Button,
  Card,
  Dropdown,
  Form,
  Input,
  Progress,
  Select,
  Tag,
  Typography,
  Upload,
} from 'antd'
import {
  ArrowUpOutlined,
  DeleteOutlined,
  EditOutlined,
  FileTextOutlined,
  MessageOutlined,
  MoreOutlined,
  PaperClipOutlined,
  PlusOutlined,
  SearchOutlined,
  StarOutlined,
  UploadOutlined,
} from '@ant-design/icons'
import { useNavigate, useParams } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'
// import { Project } from '../../types/api/projects' // Unused but may be needed later
import { useProjectsStore } from '../../store/projects'

const { Title, Text } = Typography
const { TextArea } = Input

interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: string
}

export const ProjectDetailsPage: React.FC = () => {
  const { message } = App.useApp()
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()

  // Projects store
  const {
    currentProject,
    documents,
    conversations,
    loading,
    uploading,
    error,
    loadProjectDetails,
    updateProject,
    uploadDocument,
    clearError,
  } = useProjectsStore(
    useShallow(state => ({
      currentProject: state.currentProject,
      documents: state.documents,
      conversations: state.conversations,
      loading: state.loading,
      uploading: state.uploading,
      updating: state.updating,
      error: state.error,
      loadProjectDetails: state.loadProjectDetails,
      updateProject: state.updateProject,
      uploadDocument: state.uploadDocument,
      clearError: state.clearError,
    })),
  )

  // Chat state
  const [chatInput, setChatInput] = useState('')
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: '1',
      role: 'assistant',
      content: 'How can I help you today?',
      timestamp: new Date().toISOString(),
    },
  ])
  const [selectedAssistant, setSelectedAssistant] = useState('Claude Sonnet 4')

  // Project knowledge sidebar state
  const [editingDescription, setEditingDescription] = useState(false)
  const [descriptionForm] = Form.useForm()

  useEffect(() => {
    if (projectId) {
      loadProjectDetails(projectId).catch((error: any) => {
        message.error(error?.message || 'Failed to fetch project details')
        navigate('/projects')
      })
    }
  }, [projectId, loadProjectDetails])

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      clearError()
    }
  }, [error, message, clearError])

  const handleSendMessage = () => {
    if (!chatInput.trim()) return

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: chatInput,
      timestamp: new Date().toISOString(),
    }

    setMessages([...messages, userMessage])
    setChatInput('')

    // Simulate assistant response
    setTimeout(() => {
      const assistantMessage: ChatMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: `I understand you're asking about "${chatInput}". Based on your project documents and context, I can help you with that. This is a simulated response for demonstration purposes.`,
        timestamp: new Date().toISOString(),
      }
      setMessages(prev => [...prev, assistantMessage])
    }, 1000)
  }

  const handleFileUpload = async (file: any) => {
    if (!currentProject) return

    try {
      await uploadDocument(currentProject.id, file)
      message.success('Document uploaded successfully')
    } catch (error) {
      console.error('Failed to upload document:', error)
      // Error is handled by the store
    }
  }

  const handleUpdateDescription = async (values: { description: string }) => {
    if (!currentProject) return

    try {
      await updateProject(currentProject.id, {
        description: values.description,
      })
      setEditingDescription(false)
      message.success('Description updated successfully')
    } catch (error) {
      console.error('Failed to update description:', error)
      // Error is handled by the store
    }
  }

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
  }

  if (loading || !currentProject) {
    return (
      <div className="p-6 flex justify-center min-h-screen">
        <div className="w-full max-w-7xl">
          <Text>Loading...</Text>
        </div>
      </div>
    )
  }

  return (
    <div className="p-6 flex justify-center min-h-screen">
      <div className="w-full max-w-7xl flex">
        {/* Main Chat Area */}
        <div className="flex-1 flex flex-col">
          {/* Header */}
          <div className="border-b px-6 py-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Title level={4} className="!mb-0">
                {currentProject.name}
              </Title>
              <Button icon={<StarOutlined />} type="text" className="text-xs" />
              <Tag color="default">
                {currentProject.is_private ? 'Private' : 'Public'}
              </Tag>
            </div>
            <Dropdown
              menu={{
                items: [
                  {
                    key: 'edit',
                    icon: <EditOutlined />,
                    label: 'Edit project',
                  },
                  {
                    key: 'delete',
                    icon: <DeleteOutlined />,
                    label: 'Delete project',
                    danger: true,
                  },
                ],
              }}
            >
              <Button icon={<MoreOutlined />} type="text" />
            </Dropdown>
          </div>

          {/* Chat Messages */}
          <div className="flex-1 p-6 overflow-y-auto">
            <div className="max-w-3xl mx-auto space-y-4">
              {messages.map(msg => (
                <div
                  key={msg.id}
                  className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
                >
                  <div className="max-w-2xl px-4 py-3 rounded-lg">
                    <div className="whitespace-pre-wrap">{msg.content}</div>
                    <Text type="secondary" className="text-xs mt-1">
                      {new Date(msg.timestamp).toLocaleTimeString()}
                    </Text>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Chat Input */}
          <div className="border-t px-6 py-4">
            <div className="max-w-3xl mx-auto">
              <div className="flex items-center gap-2 mb-3">
                <Button
                  icon={<PlusOutlined />}
                  type="text"
                  className="text-xs"
                />
                <Button
                  icon={<PaperClipOutlined />}
                  type="text"
                  className="text-xs"
                />
                <Button
                  icon={<SearchOutlined />}
                  type="text"
                  className="text-xs"
                >
                  Research
                </Button>
                <div className="ml-auto flex items-center gap-2">
                  <Select
                    value={selectedAssistant}
                    onChange={setSelectedAssistant}
                    style={{ width: 150 }}
                    className="text-xs"
                    options={[
                      { label: 'Claude Sonnet 4', value: 'Claude Sonnet 4' },
                      { label: 'GPT-4', value: 'GPT-4' },
                      { label: 'Gemini Pro', value: 'Gemini Pro' },
                    ]}
                  />
                  <Button
                    type="primary"
                    icon={<ArrowUpOutlined />}
                    className="text-xs"
                    onClick={handleSendMessage}
                    disabled={!chatInput.trim()}
                  />
                </div>
              </div>
              <TextArea
                value={chatInput}
                onChange={e => setChatInput(e.target.value)}
                placeholder="How can I help you today?"
                autoSize={{ minRows: 1, maxRows: 4 }}
                onPressEnter={e => {
                  if (!e.shiftKey) {
                    e.preventDefault()
                    handleSendMessage()
                  }
                }}
                className="resize-none"
                style={{ fontSize: '15px' }}
              />
            </div>
          </div>
        </div>

        {/* Project Knowledge Sidebar */}
        <div className="w-80 flex flex-col">
          <div className="p-4">
            <div className="flex items-center justify-between mb-3">
              <Text strong style={{ fontSize: '15px' }}>
                Project knowledge
              </Text>
              <Button icon={<PlusOutlined />} type="text" className="text-xs" />
            </div>

            {/* Project Description */}
            <div className="mb-4">
              {editingDescription ? (
                <Form
                  form={descriptionForm}
                  onFinish={handleUpdateDescription}
                  initialValues={{
                    description: currentProject.description || '',
                  }}
                >
                  <Form.Item name="description" className="!mb-2">
                    <TextArea
                      rows={3}
                      placeholder="Describe your project..."
                      autoFocus
                    />
                  </Form.Item>
                  <div className="flex gap-2">
                    <Button
                      className="text-xs"
                      htmlType="submit"
                      type="primary"
                    >
                      Save
                    </Button>
                    <Button
                      className="text-xs"
                      onClick={() => setEditingDescription(false)}
                    >
                      Cancel
                    </Button>
                  </div>
                </Form>
              ) : (
                <div
                  className="cursor-pointer p-2 rounded"
                  onClick={() => setEditingDescription(true)}
                >
                  <Text>
                    {currentProject.description ||
                      '"This project is to response to reviewer comment for the..."'}
                  </Text>
                  <Button type="link" className="text-xs !p-0 !h-auto !ml-1">
                    Edit
                  </Button>
                </div>
              )}
            </div>

            {/* Progress Bar */}
            <div>
              <Progress
                percent={3}
                showInfo={false}
                className="text-xs"
                strokeColor="#1890ff"
                trailColor="#e0e0e0"
              />
              <Text type="secondary" className="text-xs">
                3% of project capacity used
              </Text>
            </div>
          </div>

          {/* Document List */}
          <div className="flex-1 overflow-y-auto p-4">
            <div className="mb-6">
              <div className="flex items-center justify-between mb-3">
                <Text strong>Documents</Text>
                <Upload
                  beforeUpload={file => {
                    handleFileUpload(file)
                    return false
                  }}
                  showUploadList={false}
                >
                  <Button
                    icon={<UploadOutlined />}
                    className="text-xs"
                    loading={uploading}
                  >
                    Upload
                  </Button>
                </Upload>
              </div>

              <div className="space-y-2">
                {documents.map(doc => (
                  <Card
                    key={doc.id}
                    className="text-xs cursor-pointer hover:shadow-sm"
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex items-center gap-2 flex-1">
                        <FileTextOutlined />
                        <div className="flex-1 min-w-0">
                          <div className="font-medium text-sm truncate">
                            {doc.file_name}
                          </div>
                          <Text type="secondary" className="text-xs">
                            {formatFileSize(doc.file_size)} •{' '}
                            {doc.upload_status.toLowerCase()}
                          </Text>
                        </div>
                      </div>
                      <Tag color="default" className="uppercase">
                        {doc.file_name.split('.').pop() || 'FILE'}
                      </Tag>
                    </div>
                  </Card>
                ))}
              </div>

              {documents.length === 0 && (
                <div className="text-center py-8">
                  <FileTextOutlined className="text-3xl mb-2" />
                  <Text type="secondary" className="block text-sm">
                    No documents uploaded yet
                  </Text>
                  <Upload
                    beforeUpload={file => {
                      handleFileUpload(file)
                      return false
                    }}
                    showUploadList={false}
                  >
                    <Button className="text-xs mt-2">
                      Upload first document
                    </Button>
                  </Upload>
                </div>
              )}
            </div>

            {/* Recent Conversations */}
            <div>
              <div className="flex items-center justify-between mb-3">
                <Text strong>Recent conversations</Text>
                <Button type="link" className="text-xs !p-0">
                  View all
                </Button>
              </div>

              <div className="space-y-2">
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    Academic Manuscript Cover Letter Revision
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 9 hours ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    Academic Paper Cover Letter LaTeX
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 9 hours ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    Reviewer Feedback: Extensibility Response
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 1 day ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    Scalability Analysis for Single-Cell Datasets
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 4 days ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    CytoAnalyst: Single-Cell Data Platform
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 5 days ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    Manuscript Reviewer Response Draft
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 5 days ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    CytoAnalyst Tool Comparison Table
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 5 days ago
                  </div>
                </div>
                <div className="text-sm text-gray-600 py-2">
                  <div className="font-medium">
                    CytoAnalyst Paper Review Improvement
                  </div>
                  <div className="text-xs text-gray-400">
                    Last message 5 days ago
                  </div>
                </div>
              </div>

              {conversations.length === 0 && (
                <div className="text-center py-4">
                  <MessageOutlined className="text-xl text-gray-300 mb-2" />
                  <Text type="secondary" className="block text-sm">
                    No conversations yet
                  </Text>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
