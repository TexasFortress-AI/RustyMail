
import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useStats } from '@/dashboard/api/hooks';
import { formatDistanceToNow } from 'date-fns';
import { ActivitySquare, Zap, ServerCrash } from 'lucide-react';
import { 
  LineChart, 
  Line, 
  XAxis, 
  YAxis, 
  Tooltip, 
  ResponsiveContainer,
  CartesianGrid
} from 'recharts';
import { format } from 'date-fns';

const StatsPanel: React.FC = () => {
  const { data: stats, isLoading, isError } = useStats();

  // Format chart data
  const chartData = stats?.requestRate.map(point => ({
    time: format(new Date(point.timestamp), 'HH:mm'),
    value: point.value
  }));

  // Health status color and icon
  const getHealthStatus = () => {
    if (!stats) return { color: 'text-muted-foreground', icon: ServerCrash, text: 'Unknown' };
    
    switch (stats.systemHealth.status) {
      case 'healthy':
        return { color: 'text-green-500', icon: Zap, text: 'Healthy' };
      case 'degraded':
        return { color: 'text-amber-500', icon: Zap, text: 'Degraded' };
      case 'critical':
        return { color: 'text-red-500', icon: ServerCrash, text: 'Critical' };
      default:
        return { color: 'text-muted-foreground', icon: ServerCrash, text: 'Unknown' };
    }
  };

  const healthStatus = getHealthStatus();
  const StatusIcon = healthStatus.icon;

  return (
    <Card className="shadow-sm transition-all duration-200 animate-fade-in glass-panel" data-testid="stats-panel">
      <CardHeader className="pb-2">
        <CardTitle className="text-lg font-medium flex items-center justify-between">
          <span>System Statistics</span>
          {stats && (
            <span className="text-xs text-muted-foreground">
              Updated {formatDistanceToNow(new Date(stats.lastUpdated), { addSuffix: true })}
            </span>
          )}
        </CardTitle>
      </CardHeader>
      
      <CardContent className="pb-4">
        {isLoading ? (
          <div className="h-64 flex items-center justify-center">
            <div className="animate-pulse flex flex-col items-center gap-3">
              <div className="h-8 w-24 bg-muted rounded"></div>
              <div className="h-40 w-full bg-muted rounded"></div>
            </div>
          </div>
        ) : isError ? (
          <div className="h-64 flex items-center justify-center">
            <div className="text-destructive flex flex-col items-center">
              <ServerCrash className="h-12 w-12 mb-2" />
              <p className="text-sm">Failed to load statistics</p>
            </div>
          </div>
        ) : (
          <div className="space-y-6">
            <div className="grid grid-cols-2 gap-4">
              {/* Connection count */}
              <div className="rounded-lg bg-card/50 p-4 flex flex-col items-center justify-center animate-slide-up" style={{ animationDelay: '0.1s' }}>
                <div className="text-muted-foreground text-sm mb-2 flex items-center">
                  <ActivitySquare className="h-4 w-4 mr-1" />
                  <span>Active Connections</span>
                </div>
                <div className="text-3xl font-semibold">{stats?.activeConnections}</div>
              </div>
              
              {/* System health */}
              <div className="rounded-lg bg-card/50 p-4 flex flex-col items-center justify-center animate-slide-up" style={{ animationDelay: '0.2s' }}>
                <div className="text-muted-foreground text-sm mb-2 flex items-center">
                  <StatusIcon className="h-4 w-4 mr-1" />
                  <span>System Health</span>
                </div>
                <div className={`text-xl font-semibold ${healthStatus.color} flex items-center`}>
                  <StatusIcon className="h-5 w-5 mr-1" />
                  <span>{healthStatus.text}</span>
                </div>
                <div className="flex gap-3 mt-1 text-xs">
                  <span className="text-muted-foreground">
                    CPU: <span className="font-medium">{stats?.systemHealth.cpuUsage}%</span>
                  </span>
                  <span className="text-muted-foreground">
                    MEM: <span className="font-medium">{stats?.systemHealth.memoryUsage}%</span>
                  </span>
                </div>
              </div>
            </div>
            
            {/* Request rate chart */}
            <div className="rounded-lg bg-card/50 p-4 h-52 animate-slide-up" style={{ animationDelay: '0.3s' }}>
              <div className="text-muted-foreground text-sm mb-3 flex items-center">
                <Zap className="h-4 w-4 mr-1" />
                <span>Request Rate (last 2 hours)</span>
              </div>
              <div className="h-36">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={chartData}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#a1a1aa20" />
                    <XAxis 
                      dataKey="time"
                      stroke="#a1a1aa60"
                      fontSize={10}
                      tickLine={false}
                      axisLine={false}
                    />
                    <YAxis 
                      stroke="#a1a1aa60"
                      fontSize={10}
                      tickLine={false}
                      axisLine={false}
                      width={30}
                    />
                    <Tooltip 
                      contentStyle={{ 
                        background: 'rgba(255, 255, 255, 0.8)', 
                        borderRadius: '8px',
                        border: '1px solid rgba(0, 0, 0, 0.1)',
                        fontSize: '12px'
                      }}
                    />
                    <Line 
                      type="monotone" 
                      dataKey="value" 
                      stroke="hsl(var(--primary))" 
                      strokeWidth={2}
                      dot={false}
                      activeDot={{ r: 4 }}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
};

export default StatsPanel;
