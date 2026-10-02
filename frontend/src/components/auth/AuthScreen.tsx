import React, { useState } from 'react';
import { useAuth } from '../../context/AuthContext';
import { AuroraBackground } from '../ui/AuroraBackground';
import { SpotlightCard } from '../ui/SpotlightCard';
import { Badge } from '../ui/Badge';
import { Mail, Lock, User, ArrowRight, Sparkles, AlertCircle, Eye, EyeOff, ShieldCheck, Zap } from 'lucide-react';

export const AuthScreen: React.FC = () => {
  const { signIn, signUp } = useAuth();
  const [mode, setMode] = useState<'signin' | 'signup'>('signin');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [fullName, setFullName] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMessage(null);

    if (!email || !password) {
      setError('Please fill in both email and password.');
      return;
    }

    setLoading(true);
    try {
      if (mode === 'signin') {
        const { error: err } = await signIn(email, password);
        if (err) {
          setError(err.message || 'Invalid email or password.');
        }
      } else {
        const { error: err, data } = await signUp(email, password, fullName);
        if (err) {
          setError(err.message || 'Registration failed.');
        } else if (data && !data.session) {
          setSuccessMessage('Account created! Please check your email to verify your address, or sign in.');
          setMode('signin');
        }
      }
    } catch (err: any) {
      setError(err?.message || 'An unexpected error occurred.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuroraBackground className="w-screen h-screen flex items-center justify-center bg-[#09090f] relative overflow-hidden px-4">
      {/* Main Obsidian Spotlight Auth Card */}
      <SpotlightCard
        spotlightColor="rgba(168, 85, 247, 0.15)"
        style={{
          width: '100%',
          maxWidth: '430px',
          padding: '32px',
          borderRadius: '20px',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.7)',
        }}
      >
        {/* Header with Logo */}
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center', marginBottom: '22px' }}>
          <div
            style={{
              width: '54px',
              height: '54px',
              borderRadius: '16px',
              backgroundColor: 'rgba(168, 85, 247, 0.15)',
              border: '1px solid rgba(168, 85, 247, 0.35)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              marginBottom: '12px',
            }}
          >
            <Sparkles style={{ width: '26px', height: '26px', color: '#c084fc' }} />
          </div>
          <h1 style={{ margin: 0, fontSize: '24px', fontWeight: 800, color: '#f0eff4', letterSpacing: '-0.02em' }}>
            Welcome to <span style={{ color: 'var(--accent-purple)' }}>Rift</span>
          </h1>
          <p style={{ margin: '6px 0 0', fontSize: '12px', color: '#9c97aa' }}>
            {mode === 'signin' ? 'Sign in to access your desktop dictation studio' : 'Create an account to securely access Rift Cloud AI'}
          </p>

          <div style={{ display: 'flex', gap: '6px', marginTop: '12px' }}>
            <Badge variant="success" size="sm">
              <Zap className="w-3 h-3 text-emerald-400" /> ~85ms Groq LPU
            </Badge>
            <Badge variant="violet" size="sm">
              <ShieldCheck className="w-3 h-3 text-purple-400" /> Local-First
            </Badge>
          </div>
        </div>

        {/* Mode Toggle Tabs */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', padding: '3px', backgroundColor: '#101018', borderRadius: '10px', marginBottom: '20px' }}>
          <button
            type="button"
            onClick={() => { setMode('signin'); setError(null); }}
            style={{
              padding: '7px',
              fontSize: '12px',
              fontWeight: 600,
              borderRadius: '8px',
              border: 'none',
              backgroundColor: mode === 'signin' ? 'var(--accent-purple)' : 'transparent',
              color: mode === 'signin' ? '#0c0c12' : '#9c97aa',
              cursor: 'pointer',
              transition: 'all 0.15s ease',
            }}
          >
            Sign In
          </button>
          <button
            type="button"
            onClick={() => { setMode('signup'); setError(null); }}
            style={{
              padding: '7px',
              fontSize: '12px',
              fontWeight: 600,
              borderRadius: '8px',
              border: 'none',
              backgroundColor: mode === 'signup' ? 'var(--accent-purple)' : 'transparent',
              color: mode === 'signup' ? '#0c0c12' : '#9c97aa',
              cursor: 'pointer',
              transition: 'all 0.15s ease',
            }}
          >
            Create Account
          </button>
        </div>

        {/* Feedback Alerts */}
        {error && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '10px 12px', borderRadius: '8px', backgroundColor: 'rgba(239, 68, 68, 0.12)', border: '1px solid rgba(239, 68, 68, 0.25)', color: '#f87171', fontSize: '12px', marginBottom: '16px' }}>
            <AlertCircle style={{ width: '15px', height: '15px', flexShrink: 0 }} />
            <span>{error}</span>
          </div>
        )}
        {successMessage && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '10px 12px', borderRadius: '8px', backgroundColor: 'rgba(52, 211, 153, 0.12)', border: '1px solid rgba(52, 211, 153, 0.25)', color: '#34d399', fontSize: '12px', marginBottom: '16px' }}>
            <Sparkles style={{ width: '15px', height: '15px', flexShrink: 0 }} />
            <span>{successMessage}</span>
          </div>
        )}

        {/* Form Inputs */}
        <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
          {mode === 'signup' && (
            <div>
              <label style={{ fontSize: '11.5px', color: '#9c97aa', fontWeight: 500, display: 'block', marginBottom: '4px' }}>
                Full Name
              </label>
              <div style={{ position: 'relative' }}>
                <User style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', width: '15px', height: '15px', color: '#6e6a7d' }} />
                <input
                  type="text"
                  placeholder="Sai Srikar"
                  value={fullName}
                  onChange={(e) => setFullName(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '9px 12px 9px 36px',
                    borderRadius: '8px',
                    backgroundColor: '#101018',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    color: '#f0eff4',
                    fontSize: '13px',
                    outline: 'none',
                    boxSizing: 'border-box',
                  }}
                />
              </div>
            </div>
          )}

          <div>
            <label style={{ fontSize: '11.5px', color: '#9c97aa', fontWeight: 500, display: 'block', marginBottom: '4px' }}>
              Email Address
            </label>
            <div style={{ position: 'relative' }}>
              <Mail style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', width: '15px', height: '15px', color: '#6e6a7d' }} />
              <input
                type="email"
                placeholder="name@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                style={{
                  width: '100%',
                  padding: '9px 12px 9px 36px',
                  borderRadius: '8px',
                  backgroundColor: '#101018',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  color: '#f0eff4',
                  fontSize: '13px',
                  outline: 'none',
                  boxSizing: 'border-box',
                }}
                required
              />
            </div>
          </div>

          <div>
            <label style={{ fontSize: '11.5px', color: '#9c97aa', fontWeight: 500, display: 'block', marginBottom: '4px' }}>
              Password
            </label>
            <div style={{ position: 'relative' }}>
              <Lock style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', width: '15px', height: '15px', color: '#6e6a7d' }} />
              <input
                type={showPassword ? 'text' : 'password'}
                placeholder="••••••••"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                style={{
                  width: '100%',
                  padding: '9px 40px 9px 36px',
                  borderRadius: '8px',
                  backgroundColor: '#101018',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  color: '#f0eff4',
                  fontSize: '13px',
                  outline: 'none',
                  boxSizing: 'border-box',
                }}
                required
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                style={{
                  position: 'absolute',
                  right: '10px',
                  top: '50%',
                  transform: 'translateY(-50%)',
                  background: 'transparent',
                  border: 'none',
                  color: '#6e6a7d',
                  cursor: 'pointer',
                  padding: '4px',
                }}
              >
                {showPassword ? <EyeOff style={{ width: '15px', height: '15px' }} /> : <Eye style={{ width: '15px', height: '15px' }} />}
              </button>
            </div>
          </div>

          <button
            type="submit"
            disabled={loading}
            style={{
              marginTop: '6px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '7px',
              padding: '10px 16px',
              borderRadius: '8px',
              backgroundColor: 'var(--accent-purple)',
              color: '#0c0c12',
              fontSize: '13px',
              fontWeight: 700,
              border: 'none',
              cursor: loading ? 'not-allowed' : 'pointer',
              opacity: loading ? 0.7 : 1,
              transition: 'opacity 0.15s ease',
            }}
          >
            {loading ? 'Authenticating...' : mode === 'signin' ? 'Sign In to Rift' : 'Create Free Account'}
            {!loading && <ArrowRight style={{ width: '15px', height: '15px' }} />}
          </button>
        </form>
      </SpotlightCard>
    </AuroraBackground>
  );
};
