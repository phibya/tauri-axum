import { memo } from 'react'
import { Col, Row, Select, Space, Typography } from 'antd'
import { useTranslation } from 'react-i18next'
import { RobotOutlined } from '@ant-design/icons'
import { Assistant } from '../../types/api/assistant'
import { ModelProvider } from '../../types/api/modelProvider'
import { ChatInput } from './ChatInput'

const { Text } = Typography
const { Option } = Select

interface ChatWelcomeProps {
  selectedAssistant: string | null
  selectedModel: string | null
  assistants: Assistant[]
  modelProviders: ModelProvider[]
  onAssistantChange: (assistantId: string) => void
  onModelChange: (modelId: string) => void
  onSend: (message: string) => void | Promise<void>
}

export const ChatWelcome = memo(function ChatWelcome({
  selectedAssistant,
  selectedModel,
  assistants,
  modelProviders,
  onAssistantChange,
  onModelChange,
  onSend,
}: ChatWelcomeProps) {
  const { t } = useTranslation()

  return (
    <div className="flex flex-col h-full">
      {/* Header with model selection */}
      <div className="px-4 sm:px-6 py-4">
        <Row gutter={16} align="middle">
          <Col xs={24} sm={12} md={8}>
            <Select
              value={selectedAssistant}
              onChange={onAssistantChange}
              placeholder={t('chat.selectAssistant')}
              className="w-full"
              showSearch
              optionFilterProp="children"
            >
              {assistants.map(assistant => (
                <Option key={assistant.id} value={assistant.id}>
                  <Space>
                    <RobotOutlined />
                    {assistant.name}
                  </Space>
                </Option>
              ))}
            </Select>
          </Col>
          <Col xs={24} sm={12} md={8}>
            <Select
              value={selectedModel}
              onChange={onModelChange}
              placeholder={t('chat.selectModel')}
              className="w-full"
              showSearch
              optionFilterProp="children"
            >
              {modelProviders.map(provider => (
                <Select.OptGroup key={provider.id} label={provider.name}>
                  {provider.models.map(model => (
                    <Option
                      key={`${provider.id}:${model.id}`}
                      value={`${provider.id}:${model.id}`}
                    >
                      {model.alias}
                    </Option>
                  ))}
                </Select.OptGroup>
              ))}
            </Select>
          </Col>
        </Row>
      </div>

      {/* Welcome message */}
      <div className="flex flex-col items-center justify-center flex-1 text-center p-8">
        <div className="mb-8">
          <div className="text-3xl font-light mb-4">
            {t('chat.placeholderWelcome')}
          </div>
        </div>

        <div className="w-full max-w-2xl">
          <ChatInput
            onSend={onSend}
            placeholder={t('chat.placeholder')}
            disabled={!selectedAssistant || !selectedModel}
          />

          {(!selectedAssistant || !selectedModel) && (
            <div className="mt-4">
              <Text type="secondary" className="text-sm">
                {t('chat.noAssistantSelected')}
              </Text>
            </div>
          )}
        </div>
      </div>
    </div>
  )
})
