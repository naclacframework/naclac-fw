import re
import sys

def process_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    blocks = content.split('// ===========================================================================\n// Zero-copy branch')
    if len(blocks) != 2:
        # try windows newlines
        blocks = content.split('// ===========================================================================\n// Zero-copy branch'.replace('\n', '\r\n'))
        if len(blocks) != 2:
            print('Could not find split in ' + filepath)
            return

    borsh_part = blocks[0]
    
    # We want to find each block starting with #[cfg(all(feature = "borsh"... 
    # until the next #[cfg or end of borsh_part.
    # Better yet, since we know it's just the whole borsh_part after the header, we can just replace on borsh_part directly!
    
    # Just take everything from the first #[cfg(all(feature = "borsh", not(feature = "pinocchio")))] to the end of borsh_part
    idx = borsh_part.find('#[cfg(all(feature = "borsh"')
    if idx == -1:
        print('Could not find borsh cfg in ' + filepath)
        return
        
    header = borsh_part[:idx]
    borsh_code = borsh_part[idx:]
    
    w_block = borsh_code.replace('feature = "borsh"', 'feature = "wincode"')
    w_block = w_block.replace('BorshDeserialize + BorshSerialize', 'crate::prelude::wincode::DeserializeOwned + crate::prelude::wincode::Serialize')
    w_block = w_block.replace('BorshDeserialize', 'crate::prelude::wincode::DeserializeOwned')
    w_block = w_block.replace('BorshSerialize', 'crate::prelude::wincode::Serialize')
    w_block = w_block.replace('T::deserialize(&mut reader)', 'crate::prelude::wincode::deserialize_from(&mut reader)')
    w_block = w_block.replace('self.data\n            .serialize(&mut writer)', 'crate::prelude::wincode::serialize_into(&mut writer, &self.data)')
    w_block = w_block.replace('self.data.serialize(&mut writer)', 'crate::prelude::wincode::serialize_into(&mut writer, &self.data)')
    w_block = w_block.replace('self.data\r\n            .serialize(&mut writer)', 'crate::prelude::wincode::serialize_into(&mut writer, &self.data)')
    
    wincode_part = '\n// ===========================================================================\n// Wincode branch\n// ===========================================================================\n\n' + w_block
    
    # Now we need to update the top level zero_copy cfg
    rest = blocks[1].replace('not(feature = "borsh")', 'not(any(feature = "borsh", feature = "wincode"))')
    
    new_content = borsh_part + wincode_part + '\n// ===========================================================================\n// Zero-copy branch' + rest
    
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print('Patched ' + filepath)

process_file('naclac-core/src/wrappers/accounts/account.rs')
process_file('naclac-core/src/wrappers/accounts/interface_account.rs')
