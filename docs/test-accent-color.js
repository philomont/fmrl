// Test script for accent color switching
// Run in browser console at http://localhost:3000

function testAccentColorSwitching() {
    const results = [];
    const themes = ['default', 'retro', 'vintage', 'neon', 'zen'];
    const expectedAccents = {
        default: 'rgb(180, 30, 30)',    // #b41e1e
        retro: 'rgb(255, 123, 114)',  // #ff7b72
        vintage: 'rgb(139, 0, 0)',    // #8b0000
        neon: 'rgb(255, 0, 110)',     // #ff006e
        zen: 'rgb(233, 30, 99)',      // #e91e63
    };

    function getAccentSwatchColor() {
        const accentSwatch = document.querySelector('.swatch[data-idx="2"]');
        return accentSwatch ? accentSwatch.style.backgroundColor : 'not found';
    }

    function getAgeButtonColor() {
        const ageBtn = document.getElementById('btn-age');
        if (!ageBtn) return 'not found';
        const style = window.getComputedStyle(ageBtn);
        return style.backgroundColor;
    }

    console.log('=== Accent Color Switching Test ===\n');

    themes.forEach(theme => {
        // Set theme
        document.getElementById('theme-select').value = theme;
        document.getElementById('theme-select').dispatchEvent(new Event('change'));

        // Get actual colors
        const swatchColor = getAccentSwatchColor();
        const buttonColor = getAgeButtonColor();
        const expected = expectedAccents[theme];

        const swatchMatch = swatchColor === expected;
        const testPassed = swatchMatch;

        results.push({
            theme: theme,
            expected: expected,
            swatch: swatchColor,
            button: buttonColor,
            passed: testPassed
        });

        console.log(`Theme: ${theme}`);
        console.log(`  Expected accent: ${expected}`);
        console.log(`  Swatch color: ${swatchColor} ${swatchMatch ? '✓' : '✗'}`);
        console.log(`  Age button: ${buttonColor}`);
        console.log(`  Status: ${testPassed ? 'PASS' : 'FAIL'}`);
        console.log('');
    });

    // Summary
    const passedCount = results.filter(r => r.passed).length;
    console.log(`=== Results: ${passedCount}/${results.length} tests passed ===`);

    if (passedCount === results.length) {
        console.log('✓ All accent colors are switching correctly!');
    } else {
        console.log('✗ Some accent colors are not changing:');
        results.filter(r => !r.passed).forEach(r => {
            console.log(`  - ${r.theme}: expected ${r.expected}, got ${r.swatch}`);
        });
    }

    return results;
}

// Run the test
testAccentColorSwitching();
