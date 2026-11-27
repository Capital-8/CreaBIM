import asyncio
import json
import os
import sys
import unittest

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

from test_live import SERVER, TOKEN


class LiveOpeningsTest(unittest.IsolatedAsyncioTestCase):
    async def test_dispatches_building_element_commands(self):
        requests = []

        async def handle(reader, writer):
            requests.append(json.loads(await reader.readline()))
            writer.write(b'{"ok": true, "result": {"revision": 8}}\n')
            await writer.drain()
            writer.close()
            await writer.wait_closed()

        edit = {'document_id': 'doc', 'expected_revision': 7}
        slab = {'level_id': 'level', 'boundary': [[0, 0], [3, 0], [0, 3]]}
        typed = {'wall_id': 'host', 'center': 2., 'opening_type_id': 'type'}
        sized = {'wall_id': 'host', 'name': 'Opening', 'offset': 1., 'width': 1.,
                 'height': 1., 'sill': 1.}
        cases = (
            ('create_door', 'create_door', edit, typed),
            ('create_door', 'create_door', edit, {**sized, 'sill': 0.}),
            ('update_door', 'update_door', edit,
             {'element_id': 'door', 'frame_width': 0.08, 'frame_depth': 0.15,
              'flip_hand': False, 'flip_facing': True, 'clear_material': True}),
            ('create_window', 'create_window', edit, typed),
            ('create_window', 'create_window', edit, sized),
            ('update_window', 'update_window', edit,
             {'element_id': 'window', 'frame_width': 0.05, 'frame_depth': 0.09,
              'panes_across': 3, 'panes_up': 2, 'flip_facing': False}),
            ('create_slab', 'create_slab', edit, {**slab, 'name': 'Slab', 'thickness': 0.15}),
            ('create_slab', 'create_slab', edit, slab),
            ('update_slab', 'update_slab', edit, {'element_id': 'slab', 'top_offset': 0.12}),
            ('create_stair', 'create_stair', edit,
             {'level_id': 'level', 'start': [0, 0], 'toward': [1, 0]}),
            ('update_stair', 'update_stair', edit, {'element_id': 'stair', 'width': 1.4}),
            ('create_railing', 'create_railing', edit,
             {'level_id': 'level', 'path': [[0, 0], [4, 0], [4, 3]]}),
            ('update_railing', 'update_railing', edit, {'element_id': 'rail', 'height': 1.1}),
        )
        bridge = await asyncio.start_server(handle, '127.0.0.1', 0)
        try:
            port = str(bridge.sockets[0].getsockname()[1])
            params = StdioServerParameters(command=sys.executable, args=[str(SERVER)],
                                           env={**os.environ, 'CREABIM_MCP_PORT': port, 'CREABIM_MCP_TOKEN': TOKEN})
            async with stdio_client(params) as (reader, writer):
                async with ClientSession(reader, writer) as client:
                    await client.initialize()
                    tools = {tool.name: tool for tool in (await client.list_tools()).tools}
                    own_names = {f'live_{tool}' for tool, _, _, _ in cases}
                    self.assertEqual(len(own_names), 10)
                    self.assertTrue(own_names <= tools.keys())
                    for tool, command, identifiers, args in cases:
                        with self.subTest(tool=tool, args=args):
                            name = f'live_{tool}'
                            self.assertEqual(tools[name].annotations.destructiveHint,
                                             tool.startswith('update_'))
                            result = await client.call_tool(name, {**identifiers, **args})
                            self.assertFalse(result.isError, result)
                            self.assertEqual(requests[-1], {'token': TOKEN, 'command': command,
                                                             'args': args, **identifiers})
                    for tool, args in (('door', {**typed, 'width': 1.0}),
                                       ('window', {'wall_id': 'host', 'center': 2.0})):
                        result = await client.call_tool(f'live_create_{tool}', {**edit, **args})
                        self.assertTrue(result.isError, result)
                    self.assertEqual(len(requests), len(cases))
        finally:
            bridge.close()
            await bridge.wait_closed()
