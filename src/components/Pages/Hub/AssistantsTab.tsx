import { ClearOutlined, SearchOutlined } from '@ant-design/icons'
import { Button, Flex, Input, Select, Typography } from 'antd'
import { useMemo, useState } from 'react'
import { searchAssistants, useHubStore } from '../../../store/hub'
import { AssistantCard } from './AssistantCard'

const { Text } = Typography

export function AssistantsTab() {
  const { assistants } = useHubStore()
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [sortBy, setSortBy] = useState('popular')

  const clearAllFilters = () => {
    setSearchTerm('')
    setSelectedTags([])
  }

  // Get unique tags for filters
  const assistantTags = useMemo(() => {
    const allTags = new Set<string>()
    assistants.forEach(assistant => {
      assistant.tags.forEach(tag => allTags.add(tag))
    })
    return Array.from(allTags).sort()
  }, [assistants])

  const filteredAssistants = useMemo(() => {
    let filtered = searchAssistants(assistants, searchTerm)

    // Filter by tags
    if (selectedTags.length > 0) {
      filtered = filtered.filter(assistant =>
        selectedTags.some(tag => assistant.tags.includes(tag)),
      )
    }

    // Sort assistants
    switch (sortBy) {
      case 'popular':
        filtered.sort(
          (a, b) => (b.popularity_score || 0) - (a.popularity_score || 0),
        )
        break
      case 'name':
        filtered.sort((a, b) => a.name.localeCompare(b.name))
        break
      default:
        break
    }

    return filtered
  }, [assistants, searchTerm, selectedTags, sortBy])

  return (
    <Flex className={'flex-col gap-3'}>
      {/* Search and Filters */}
      <div>
        <Flex className="gap-3">
          <Input
            placeholder="Search assistants..."
            prefix={<SearchOutlined />}
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            allowClear
            className="flex-1"
          />
          <Select
            mode="multiple"
            placeholder="Filter by tags"
            value={selectedTags}
            onChange={setSelectedTags}
            className="flex-1"
            allowClear
            maxTagCount="responsive"
            options={assistantTags.map(tag => ({
              key: tag,
              value: tag,
              label: tag,
            }))}
          />
          <Select
            placeholder="Sort by"
            value={sortBy}
            onChange={setSortBy}
            className="flex-1"
            options={[
              { value: 'popular', label: 'Popular' },
              { value: 'name', label: 'Name' },
            ]}
          />
        </Flex>
        {(searchTerm || selectedTags.length > 0) && (
          <Flex align="center" gap={8}>
            <Text type="secondary" className="text-xs">
              Filters active:{' '}
              {[
                searchTerm && 'search',
                selectedTags.length > 0 && `${selectedTags.length} tags`,
              ]
                .filter(Boolean)
                .join(', ')}
            </Text>
            <Button
              size="small"
              type="text"
              icon={<ClearOutlined />}
              onClick={clearAllFilters}
            >
              Clear all
            </Button>
          </Flex>
        )}
      </div>

      {/* Assistants Grid */}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4">
        {filteredAssistants.map(assistant => (
          <AssistantCard key={assistant.id} assistant={assistant} />
        ))}
      </div>

      {filteredAssistants.length === 0 && (
        <div className="text-center py-12">
          <Text type="secondary">No assistants found</Text>
        </div>
      )}
    </Flex>
  )
}
